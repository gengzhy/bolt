//! 接收流水线（实施方案 7.2/7.3/8.2/8.3）。
//!
//! 由调度器驱动：FILE_META 建立临时写入器并应答空区间 →
//! DATA 按偏移写入 .tmp 临时文件 → 即时累积确认（2MB 或 100ms 先到者）→
//! FILE_DONE 全文件 BLAKE3 校验后原子落盘到保存目录。
//! 任务取消或校验失败时立即删除临时文件，保证零磁盘脏数据残留。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use lt_file::writer::FileWriter;
use lt_file::FileIdentity;
use lt_utils::constants::*;
use lt_utils::{LtError, LtResult};

use crate::protocol::Message;
use crate::session::{EngineEvent, IncomingTransfer, Session};

/// 单个入站任务（对应一次 TRANSFER_REQ）。
pub struct RecvTask {
    pub req: IncomingTransfer,
    pub total_size: u64,
    pub received_bytes: u64,
    pub completed_files_bytes: u64,
    pub ok: u32,
    pub failed: u32,
    pub files: HashMap<u32, RecvFile>,
    pub data_dir: std::path::PathBuf,
    pub rate_bytes_last: u64,
    pub rate_time_last: Instant,
    pub last_progress: Instant,
    pub smoothed_rate: f64,
}

impl RecvTask {
    pub fn new(req: IncomingTransfer, total_size: u64, data_dir: std::path::PathBuf) -> RecvTask {
        let now = Instant::now();
        RecvTask {
            req,
            total_size,
            received_bytes: 0,
            completed_files_bytes: 0,
            ok: 0,
            failed: 0,
            files: HashMap::new(),
            data_dir,
            rate_bytes_last: 0,
            rate_time_last: now,
            last_progress: now.checked_sub(Duration::from_secs(1)).unwrap_or(now),
            smoothed_rate: 0.0,
        }
    }

    /// 获取全任务当前已安全落盘并确认的字节总数（已完成文件总大小 + 活跃文件已写字节之和）
    pub fn current_done(&self) -> u64 {
        let active: u64 = self.files.values().map(|rf| rf.writer.written_bytes()).sum();
        self.completed_files_bytes.saturating_add(active).min(self.total_size)
    }

    /// 发射全任务进度事件（节流或强制发射）
    pub fn emit_progress(
        &mut self,
        session: &std::sync::Arc<Session>,
        task_session: u64,
        rel_path: Option<String>,
        force: bool,
    ) {
        let now = Instant::now();
        if !force && now.duration_since(self.last_progress) < INTERVAL_PROGRESS_EMIT {
            return;
        }
        let current_done = self.current_done();
        let dt = now.duration_since(self.rate_time_last).as_secs_f64().max(0.001);
        let instant_rate = (current_done.saturating_sub(self.rate_bytes_last)) as f64 / dt;
        self.smoothed_rate = if force {
            0.0
        } else if self.smoothed_rate <= 0.0 {
            instant_rate
        } else if instant_rate > 0.0 {
            0.7 * instant_rate + 0.3 * self.smoothed_rate
        } else {
            self.smoothed_rate * 0.8
        };
        let remaining = self.total_size.saturating_sub(current_done);
        let eta = if self.smoothed_rate > 1.0 {
            (remaining as f64 / self.smoothed_rate) as u64
        } else {
            0
        };
        session.emit(EngineEvent::Progress {
            conn_id: session.id,
            task_id: task_session,
            incoming: true,
            rel_path,
            done: current_done,
            total: self.total_size,
            rate_bps: self.smoothed_rate as u64,
            eta_secs: eta,
        });
        self.last_progress = now;
        self.rate_bytes_last = current_done;
        self.rate_time_last = now;
    }

    fn finished(&self) -> bool {
        self.ok + self.failed >= self.req.file_count
    }
}

impl Drop for RecvTask {
    fn drop(&mut self) {
        let files = std::mem::take(&mut self.files);
        for (_, rf) in files {
            rf.writer.discard();
        }
    }
}

/// 单个入站文件状态。
pub struct RecvFile {
    pub ident: FileIdentity,
    pub chunk_size: u32,
    pub pipe_id: u64,
    pub writer: FileWriter,
    pub received: u64,
    pub ack_prefix: u64,
    pub ack_bytes_since: u64,
    pub ack_last: Instant,
    pub last_progress: Instant,
    pub rate_bytes_last: u64,
    pub rate_time_last: Instant,
}

pub fn on_file_meta(
    session: &std::sync::Arc<Session>,
    recv_tasks: &mut HashMap<u64, RecvTask>,
    pipe_id: u64,
    task_session: u64,
    file_seq: u32,
    size: u64,
    mtime: i64,
    rel_path: String,
    chunk_size: u32,
) {
    let Some(task) = recv_tasks.get_mut(&task_session) else {
        let _ = session.send_file(
            pipe_id,
            task_session,
            Message::FileMetaAck {
                file_seq,
                accept: false,
                ranges: vec![],
            },
        );
        return;
    };

    let ident = FileIdentity {
        rel_path: rel_path.clone(),
        size,
        mtime_unix: mtime,
    };

    let uid_hex: String = task.req.task_uid.iter().map(|b| format!("{:02x}", b)).collect();
    let task_tmp = session.cfg.tmp_dir.join(format!("task_{uid_hex}"));
    let writer = match FileWriter::create(&task_tmp, &format!("f{file_seq}"), size) {
        Ok(w) => w,
        Err(e) => {
            let _ = session.send_file(
                pipe_id,
                task_session,
                Message::FileMetaAck {
                    file_seq,
                    accept: false,
                    ranges: vec![],
                },
            );
            session.emit(EngineEvent::Error {
                conn_id: session.id,
                task_id: Some(task_session),
                incoming: true,
                code: e.code(),
                message: format!("无法创建临时文件：{rel_path}"),
            });
            task.failed += 1;
            return;
        }
    };

    let rf = RecvFile {
        ident,
        chunk_size,
        pipe_id,
        writer,
        received: 0,
        ack_prefix: 0,
        ack_bytes_since: 0,
        ack_last: Instant::now(),
        last_progress: Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now),
        rate_bytes_last: 0,
        rate_time_last: Instant::now(),
    };
    task.files.insert(file_seq, rf);

    let _ = session.send_file(
        pipe_id,
        task_session,
        Message::FileMetaAck {
            file_seq,
            accept: true,
            ranges: vec![],
        },
    );
}

pub fn on_data(
    session: &std::sync::Arc<Session>,
    recv_tasks: &mut HashMap<u64, RecvTask>,
    pipe_id: u64,
    task_session: u64,
    file_seq: u32,
    chunk_seq: u64,
    payload: bytes::Bytes,
) {
    let Some(task) = recv_tasks.get_mut(&task_session) else {
        return;
    };
    let Some(rf) = task.files.get_mut(&file_seq) else {
        return;
    };
    if rf.pipe_id != pipe_id {
        return;
    }
    let rel_path = Some(rf.ident.rel_path.clone());

    let len = payload.len() as u64;
    let offset = chunk_seq * rf.chunk_size as u64;
    if let Err(e) = rf.writer.write_chunk(offset, &payload) {
        session.emit(EngineEvent::Error {
            conn_id: session.id,
            task_id: Some(task_session),
            incoming: true,
            code: e.code(),
            message: format!("写入分片失败：offset={offset}, len={len}"),
        });
        return;
    }
    rf.received = rf.writer.written_bytes();
    rf.ack_bytes_since += len;
    task.received_bytes += len;

    // 即时累积确认：累计达 2MB 时立即回传 ACK
    let written = rf.writer.written_bytes();
    if written > rf.ack_prefix && rf.ack_bytes_since >= ACK_THRESHOLD_BYTES {
        let _ = session.send_file(
            rf.pipe_id,
            task_session,
            Message::Ack {
                file_seq,
                acked_offset: written,
            },
        );
        rf.ack_prefix = written;
        rf.ack_bytes_since = 0;
        rf.ack_last = Instant::now();
    }

    // 进度事件（节流汇报全任务总进度）
    task.emit_progress(session, task_session, rel_path, false);
}

pub fn flush_acks(session: &std::sync::Arc<Session>, recv_tasks: &mut HashMap<u64, RecvTask>) {
    for (task_session, task) in recv_tasks.iter_mut() {
        for (seq, rf) in task.files.iter_mut() {
            let written = rf.writer.written_bytes();
            let due_time = rf.ack_last.elapsed() >= Duration::from_millis(100);
            let due_bytes = rf.ack_bytes_since >= ACK_THRESHOLD_BYTES;
            if written > rf.ack_prefix && (due_time || due_bytes) {
                let _ = session.send_file(
                    rf.pipe_id,
                    *task_session,
                    Message::Ack {
                        file_seq: *seq,
                        acked_offset: written,
                    },
                );
                rf.ack_prefix = written;
                rf.ack_bytes_since = 0;
                rf.ack_last = Instant::now();
            }
        }
    }
}

pub fn on_file_done(
    session: &std::sync::Arc<Session>,
    recv_tasks: &mut HashMap<u64, RecvTask>,
    pipe_id: u64,
    task_session: u64,
    file_seq: u32,
    hash: [u8; 32],
) {
    let Some(task) = recv_tasks.get_mut(&task_session) else {
        return;
    };

    let Some(rf) = task.files.get_mut(&file_seq) else {
        let _ = session.send_control(task_session, Message::FileDoneAck { file_seq, ok: true });
        return;
    };
    if rf.pipe_id != pipe_id {
        return;
    }
    if !rf.writer.is_complete() {
        let _ = session.send_file(
            rf.pipe_id,
            task_session,
            Message::FileDoneAck {
                file_seq,
                ok: false,
            },
        );
        return;
    }
    // 收尾确认，让发送端进度到达 100%
    let _ = session.send_file(
        rf.pipe_id,
        task_session,
        Message::Ack {
            file_seq,
            acked_offset: rf.ident.size,
        },
    );

    let rf = task.files.remove(&file_seq).expect("checked above");
    let rel = rf.ident.rel_path.clone();
    let rel_for_event = rel.clone();
    let file_size = rf.ident.size;
    let save_dir = session.cfg.save_dir.clone();
    let collision = session.cfg.collision;
    let disp_tx = session.disp_tx.clone();
    tokio::spawn(async move {
        let start = Instant::now();
        let result = tokio::task::spawn_blocking(move || {
            rf.writer
                .verify_and_place(&hash, &save_dir, &rel, collision)
        })
        .await
        .map_err(|_| LtError::Internal)
        .and_then(|r| r);
        tracing::info!(
            file_seq,
            size = file_size,
            ms = start.elapsed().as_millis() as u64,
            "file received: verify+place done"
        );
        let _ = disp_tx.send(crate::session::disp_file_verified(
            task_session,
            file_seq,
            result,
            rel_for_event,
        ));
    });
}

pub fn on_file_verified(
    session: &std::sync::Arc<Session>,
    recv_tasks: &mut HashMap<u64, RecvTask>,
    task_session: u64,
    file_seq: u32,
    result: LtResult<std::path::PathBuf>,
    rel_path: &str,
) {
    let task_opt = recv_tasks.get_mut(&task_session);
    if task_opt.is_none() {
        return;
    }
    let task = task_opt.unwrap();
    match result {
        Ok(_dest) => {
            task.ok += 1;
            if let Ok(m) = std::fs::metadata(&_dest) {
                task.completed_files_bytes += m.len();
            }
            let final_rel_path = _dest
                .strip_prefix(&session.cfg.save_dir)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.replace('\\', "/"))
                .unwrap_or_else(|| rel_path.to_string());

            task.emit_progress(session, task_session, Some(final_rel_path.clone()), true);
            let _ = session.send_control(task_session, Message::FileDoneAck { file_seq, ok: true });
            session.emit(EngineEvent::FileFinished {
                conn_id: session.id,
                task_id: task_session,
                incoming: true,
                rel_path: final_rel_path,
                ok: true,
            });
        }
        Err(e) => {
            task.failed += 1;
            task.emit_progress(session, task_session, Some(rel_path.to_string()), true);
            let _ = session.send_control(task_session, Message::FileDoneAck { file_seq, ok: false });
            session.emit(EngineEvent::FileFinished {
                conn_id: session.id,
                task_id: task_session,
                incoming: true,
                rel_path: rel_path.to_string(),
                ok: false,
            });
            session.emit(EngineEvent::Error {
                conn_id: session.id,
                task_id: Some(task_session),
                incoming: true,
                code: e.code(),
                message: format!("文件 {rel_path} 校验/落盘失败"),
            });
        }
    }

    // 磁盘复检
    let remaining = task.total_size.saturating_sub(task.received_bytes);
    if !task.finished() && !lt_file::disk::recheck(&session.cfg.save_dir, remaining) {
        session.emit(EngineEvent::Error {
            conn_id: session.id,
            task_id: Some(task_session),
            incoming: true,
            code: LtError::DiskFull.code(),
            message: "磁盘空间不足".into(),
        });
    }

    if task.finished() {
        let ok = task.ok;
        let failed = task.failed;
        let uid_hex: String = task.req.task_uid.iter().map(|b| format!("{:02x}", b)).collect();
        session.emit(EngineEvent::Summary {
            conn_id: session.id,
            task_id: task_session,
            incoming: true,
            ok,
            failed,
        });
        recv_tasks.remove(&task_session);
        let task_tmp = session.cfg.tmp_dir.join(format!("task_{uid_hex}"));
        let _ = std::fs::remove_dir_all(&task_tmp);
    }
}

/// 对端或本地取消：立即丢弃未完成的临时文件，并彻底删除任务临时目录，杜绝磁盘脏数据残留。
pub fn on_cancel(
    _session: &std::sync::Arc<Session>,
    recv_tasks: &mut HashMap<u64, RecvTask>,
    task_session: u64,
    _reason: u32,
) {
    if let Some(mut task) = recv_tasks.remove(&task_session) {
        let files = std::mem::take(&mut task.files);
        for (_, rf) in files {
            rf.writer.discard();
        }
        let uid_hex: String = task.req.task_uid.iter().map(|b| format!("{:02x}", b)).collect();
        let task_tmp = _session.cfg.tmp_dir.join(format!("task_{uid_hex}"));
        let _ = std::fs::remove_dir_all(&task_tmp);
        task.failed += task.req.file_count.saturating_sub(task.ok + task.failed);
    }
}
