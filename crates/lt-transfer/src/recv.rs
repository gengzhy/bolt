//! 接收流水线（实施方案 7.2/7.3/8.2/8.3）。
//!
//! 由调度器驱动：FILE_META 建写入器并回传已收区间（断点协商）→
//! DATA 按偏移写入 → 周期性累积确认（200ms / 8MB 先到者）→
//! FILE_DONE 全文件 BLAKE3 校验后落盘。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use lt_file::writer::FileWriter;
use lt_file::FileIdentity;
use lt_utils::{LtError, LtResult};

use crate::protocol::Message;
use crate::session::{EngineEvent, IncomingTransfer, Session};

/// 单个入站任务（对应一次 TRANSFER_REQ）。
pub struct RecvTask {
    pub req: IncomingTransfer,
    pub total_size: u64,
    pub received_bytes: u64,
    pub ok: u32,
    pub failed: u32,
    pub paused: bool,
    pub files: HashMap<u32, RecvFile>,
    pub data_dir: std::path::PathBuf,
}

impl RecvTask {
    pub fn new(req: IncomingTransfer, total_size: u64, data_dir: std::path::PathBuf) -> RecvTask {
        RecvTask {
            req,
            total_size,
            received_bytes: 0,
            ok: 0,
            failed: 0,
            paused: false,
            files: HashMap::new(),
            data_dir,
        }
    }
    fn finished(&self) -> bool {
        self.ok + self.failed >= self.req.file_count
    }
}

impl Drop for RecvTask {
    fn drop(&mut self) {
        let store = lt_file::resume_store::ResumeStore::new(&self.data_dir);
        for rf in self.files.values() {
            if rf.received > 0 {
                let _ = store.save(
                    &rf.resume_key,
                    &rf.ident.rel_path,
                    rf.ident.size,
                    rf.writer.received(),
                );
            }
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
    /// 断点缓存键（文件身份哈希）
    pub resume_key: String,
    /// 上次持久化断点的时间 / 已持久化字节数
    pub resume_save_last: Instant,
    pub resume_saved_bytes: u64,
}

#[allow(clippy::too_many_arguments)]
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
    head_hash: [u8; 16],
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
    if task.paused {
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
    }

    let ident = FileIdentity {
        rel_path: rel_path.clone(),
        size,
        mtime_unix: mtime,
        head_hash,
    };
    let store = lt_file::resume_store::ResumeStore::new(&session.cfg.data_dir);
    let key = ident.cache_key();
    let resumed = store.load(&key);
    let task_tmp = session.cfg.tmp_dir.join(format!("task_{task_session}"));
    let writer = match &resumed {
        Some(r) => FileWriter::resume(&task_tmp, &format!("f{file_seq}"), size, r.clone()),
        None => FileWriter::create(&task_tmp, &format!("f{file_seq}"), size),
    };
    let writer = match writer {
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

    let ranges_out = writer.received().intervals().to_vec();
    let received = writer.received().total_received();
    let rf = RecvFile {
        ident,
        chunk_size,
        pipe_id,
        writer,
        received,
        ack_prefix: 0,
        ack_bytes_since: 0,
        ack_last: Instant::now(),
        last_progress: Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now),
        // 断点续传时 received 已含历史字节：速率只统计本次新增
        rate_bytes_last: received,
        rate_time_last: Instant::now(),
        resume_key: key.clone(),
        resume_save_last: Instant::now(),
        resume_saved_bytes: 0,
    };
    let _ = session.send_file(
        pipe_id,
        task_session,
        Message::FileMetaAck {
            file_seq,
            accept: true,
            ranges: ranges_out,
        },
    );
    if resumed.is_some() {
        tracing::info!(file_seq, received, "resume negotiated");
    }
    task.files.insert(file_seq, rf);
}

pub fn on_data(
    session: &std::sync::Arc<Session>,
    recv_tasks: &mut HashMap<u64, RecvTask>,
    task_session: u64,
    file_seq: u32,
    chunk_seq: u64,
    payload: bytes::Bytes,
) {
    // 帧头自带任务号：直接按 (任务号, 文件序号) 两级索引。
    // 此前用全局 file_seq→task 映射，同会话并发多任务时互相覆盖，
    // 数据被路由到另一个任务的文件上，写入偏移越界报 -1「参数无效」。
    let Some(task) = recv_tasks.get_mut(&task_session) else {
        return;
    };
    let Some(rf) = task.files.get_mut(&file_seq) else {
        return;
    };

    let offset = chunk_seq * rf.chunk_size as u64;
    let len = payload.len() as u64;
    let write_res = tokio::task::block_in_place(|| rf.writer.write_chunk(offset, &payload));
    if let Err(e) = write_res {
        session.emit(EngineEvent::Error {
            conn_id: session.id,
            task_id: Some(task_session),
            incoming: true,
            code: e.code(),
            message: format!("写入失败：{}", rf.ident.rel_path),
        });
        return;
    }
    rf.received += len;
    rf.ack_bytes_since += len;
    task.received_bytes += len;

    // 即时累积确认：累计达 4MB 且存在新连续前缀时立即回传 ACK，
    // 不再被动等待 200ms 定时器，彻底消除发送端由于在途窗口打满而饥饿停等
    let prefix = rf.writer.received().contiguous_prefix();
    if prefix > rf.ack_prefix && rf.ack_bytes_since >= 4 * 1024 * 1024 {
        let _ = session.send_file(
            rf.pipe_id,
            task_session,
            Message::Ack {
                file_seq,
                acked_offset: prefix,
            },
        );
        rf.ack_prefix = prefix;
        rf.ack_bytes_since = 0;
        rf.ack_last = Instant::now();
    }

    // 进度事件（250ms 节流）
    let now = Instant::now();
    if now.duration_since(rf.last_progress) >= Duration::from_millis(250) {
        let dt = now
            .duration_since(rf.rate_time_last)
            .as_secs_f64()
            .max(0.001);
        let rate = (rf.received.saturating_sub(rf.rate_bytes_last)) as f64 / dt;
        let remaining = rf.ident.size.saturating_sub(rf.received);
        let eta = if rate > 1.0 {
            (remaining as f64 / rate) as u64
        } else {
            0
        };
        session.emit(EngineEvent::Progress {
            conn_id: session.id,
            task_id: task_session,
            incoming: true,
            rel_path: Some(rf.ident.rel_path.clone()),
            done: rf.received,
            total: rf.ident.size,
            rate_bps: rate as u64,
            eta_secs: eta,
        });
        rf.last_progress = now;
        rf.rate_bytes_last = rf.received;
        rf.rate_time_last = now;
    }
}

/// 周期刷出累积确认：每 200ms 或每 4MB（取先到者，作为尾部数据兜底）。
/// 同时按相同节流规则持久化断点缓存（重连续传的数据来源）。
pub fn flush_acks(session: &std::sync::Arc<Session>, recv_tasks: &mut HashMap<u64, RecvTask>) {
    for (task_session, task) in recv_tasks.iter_mut() {
        for (seq, rf) in task.files.iter_mut() {
            let prefix = rf.writer.received().contiguous_prefix();
            let due_time = rf.ack_last.elapsed() >= Duration::from_millis(200);
            let due_bytes = rf.ack_bytes_since >= 4 * 1024 * 1024;
            if prefix > rf.ack_prefix && (due_time || due_bytes) {
                let _ = session.send_file(
                    rf.pipe_id,
                    *task_session,
                    Message::Ack {
                        file_seq: *seq,
                        acked_offset: prefix,
                    },
                );
                rf.ack_prefix = prefix;
                rf.ack_bytes_since = 0;
                rf.ack_last = Instant::now();
            }

            // 断点持久化：每 2s 或每 8MB 新数据写一次磁盘
            let saved = rf.resume_saved_bytes;
            let new_bytes = rf.received.saturating_sub(saved);
            let due_save_time = rf.resume_save_last.elapsed() >= Duration::from_secs(2);
            let due_save_bytes = new_bytes >= 8 * 1024 * 1024;
            if new_bytes > 0 && (due_save_time || due_save_bytes) {
                let ranges = rf.writer.received();
                let _ = lt_file::resume_store::ResumeStore::new(&session.cfg.data_dir).save(
                    &rf.resume_key,
                    &rf.ident.rel_path,
                    rf.ident.size,
                    ranges,
                );
                rf.resume_saved_bytes = rf.received;
                rf.resume_save_last = Instant::now();
            }
        }
    }
}

pub fn on_file_done(
    session: &std::sync::Arc<Session>,
    recv_tasks: &mut HashMap<u64, RecvTask>,
    task_session: u64,
    file_seq: u32,
    hash: [u8; 32],
) {
    let Some(task) = recv_tasks.get_mut(&task_session) else {
        return;
    };

    let Some(rf) = task.files.get_mut(&file_seq) else {
        return;
    };
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
    // 收尾确认，让发送端进度到 100%
    let _ = session.send_file(
        rf.pipe_id,
        task_session,
        Message::Ack {
            file_seq,
            acked_offset: rf.ident.size,
        },
    );

    let rf = task.files.remove(&file_seq).expect("checked above");
    let key = rf.ident.cache_key();
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
            key,
            rel_for_event,
        ));
    });
}

#[allow(clippy::too_many_arguments)]
pub fn on_file_verified(
    session: &std::sync::Arc<Session>,
    recv_tasks: &mut HashMap<u64, RecvTask>,
    task_session: u64,
    file_seq: u32,
    result: LtResult<std::path::PathBuf>,
    resume_key: &str,
    rel_path: &str,
) {
    let task_opt = recv_tasks.get_mut(&task_session);
    if task_opt.is_none() {
        if result.is_ok() || matches!(result, Err(LtError::ChecksumMismatch)) {
            lt_file::resume_store::ResumeStore::new(&session.cfg.data_dir).remove(resume_key);
        }
        return;
    }
    let task = task_opt.unwrap();
    match result {
        Ok(_dest) => {
            task.ok += 1;
            lt_file::resume_store::ResumeStore::new(&session.cfg.data_dir).remove(resume_key);
            // FILE_DONE_ACK(ok=true)：发送端据此结束该文件
            let _ = session.send_control(task_session, Message::FileDoneAck { file_seq, ok: true });
            session.emit(EngineEvent::FileFinished {
                conn_id: session.id,
                task_id: task_session,
                incoming: true,
                rel_path: rel_path.to_string(),
                ok: true,
            });
        }
        Err(e) => {
            task.failed += 1;
            // 校验失败（内容损坏）时清除断点缓存：该缓存要么已与 tmp 文件
            // 脱节（resume 一致性检查已放弃），要么记录了坏区间，留着会让
            // 下一次同文件传输再次命中并重复失败
            if e == LtError::ChecksumMismatch {
                lt_file::resume_store::ResumeStore::new(&session.cfg.data_dir).remove(resume_key);
            }
            let _ = session.send_control(
                task_session,
                Message::FileDoneAck {
                    file_seq,
                    ok: false,
                },
            );
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

    // 磁盘复检（每完成一个文件，实施方案 8.3-2）
    let remaining = task.total_size.saturating_sub(task.received_bytes);
    if !task.finished() && !lt_file::disk::recheck(&session.cfg.save_dir, remaining) {
        task.paused = true;
        session.emit(EngineEvent::Error {
            conn_id: session.id,
            task_id: Some(task_session),
            incoming: true,
            code: LtError::DiskFull.code(),
            message: "磁盘空间不足，任务已暂停".into(),
        });
    }

    if task.finished() {
        let ok = task.ok;
        let failed = task.failed;
        session.emit(EngineEvent::Summary {
            conn_id: session.id,
            task_id: task_session,
            incoming: true,
            ok,
            failed,
        });
        recv_tasks.remove(&task_session);
        // 清理任务临时目录（应已为空）
        let task_tmp = session.cfg.tmp_dir.join(format!("task_{task_session}"));
        let _ = std::fs::remove_dir_all(&task_tmp);
    }
}

/// 对端取消：临时文件默认保留（可续传）。
/// 清理内存状态前先把已收区间写入断点缓存。
pub fn on_cancel(
    session: &std::sync::Arc<Session>,
    recv_tasks: &mut HashMap<u64, RecvTask>,
    task_session: u64,
    reason: u32,
) {
    if let Some(mut task) = recv_tasks.remove(&task_session) {
        let store = lt_file::resume_store::ResumeStore::new(&session.cfg.data_dir);
        let files = std::mem::take(&mut task.files);
        for rf in files.into_values() {
            if rf.received > 0 {
                let _ = store.save(
                    &rf.resume_key,
                    &rf.ident.rel_path,
                    rf.ident.size,
                    rf.writer.received(),
                );
            }
            if reason == 1 {
                rf.writer.discard();
                let _ = store.remove(&rf.resume_key);
            }
        }
        if reason == 1 {
            let task_tmp = session.cfg.tmp_dir.join(format!("task_{task_session}"));
            let _ = std::fs::remove_dir_all(&task_tmp);
        }
        task.failed += task.req.file_count.saturating_sub(task.ok + task.failed);
    }
}
