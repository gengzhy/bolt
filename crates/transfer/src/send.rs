//! 发送流水线（实施方案 7.2/7.3/8.1）。
//!
//! TRANSFER_REQ → 逐文件 FILE_META（带回断点区间协商）→ DATA 分片
//! （边读边算 BLAKE3，已收分片跳过）→ FILE_DONE 哈希比对。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use file::reader::FileReader;
use file::traverse::TransferItem;
use utils::constants::*;
use utils::{BtError, BtResult};

use crate::protocol::{Message, HASH_BLAKE3};
use crate::session::{EngineEvent, EventSink, Session, WaitKey};

/// 发送汇总。
#[derive(Debug, Clone, Copy)]
pub struct SendSummary {
    pub ok: u32,
    pub failed: u32,
}

/// 全任务级别的发送进度与速率追踪器（线程安全）。
/// 无论是单文件还是多文件并发/串行传输，对外均统一汇报全任务的总已完成字节与总大小，
/// 彻底消除单文件进度交错导致的进度条跳变倒退以及两端不一致。
pub struct TaskSendTracker {
    pub total_size: u64,
    pub file_dones: Vec<u64>,
    pub smoothed_rate: f64,
    pub rate_bytes_last: u64,
    pub rate_time_last: Instant,
    pub last_emit: Instant,
}

impl TaskSendTracker {
    pub fn new(total_size: u64, file_count: usize) -> Self {
        let now = Instant::now();
        Self {
            total_size,
            file_dones: vec![0; file_count],
            smoothed_rate: 0.0,
            rate_bytes_last: 0,
            rate_time_last: now,
            last_emit: now.checked_sub(Duration::from_secs(1)).unwrap_or(now),
        }
    }

    pub fn current_done(&self) -> u64 {
        self.file_dones.iter().sum::<u64>().min(self.total_size)
    }

    /// 更新某文件的已确认/已落盘进度，并在满足 250ms 节流时发射全局进度
    pub fn update_file(
        &mut self,
        file_seq: usize,
        file_done: u64,
        rel_path: &str,
        session_id: u64,
        task_id: u64,
        sink: &EventSink,
        force: bool,
    ) {
        if file_seq < self.file_dones.len() {
            self.file_dones[file_seq] = file_done;
        }
        let current_done = self.current_done();
        tracing::debug!(file_seq, file_done, ?self.file_dones, current_done, self.total_size, "TaskSendTracker update_file");
        let now = Instant::now();
        if !force && now.duration_since(self.last_emit) < INTERVAL_PROGRESS_EMIT {
            return;
        }
        let dt = now.duration_since(self.rate_time_last).as_secs_f64().max(0.001);
        let instant_rate = (current_done.saturating_sub(self.rate_bytes_last)) as f64 / dt;
        self.smoothed_rate = if force {
            0.0
        } else if self.smoothed_rate <= 0.0 {
            instant_rate
        } else if instant_rate > 0.0 {
            0.7 * instant_rate + 0.3 * self.smoothed_rate
        } else if now.duration_since(self.rate_time_last) > Duration::from_millis(1500) {
            0.0
        } else {
            self.smoothed_rate * 0.5
        };
        let remaining = self.total_size.saturating_sub(current_done);
        let eta = if self.smoothed_rate > 1.0 {
            (remaining as f64 / self.smoothed_rate) as u64
        } else {
            0
        };
        sink(EngineEvent::Progress {
            conn_id: session_id,
            task_id,
            incoming: false,
            rel_path: Some(rel_path.to_string()),
            done: current_done,
            total: self.total_size,
            rate_bps: self.smoothed_rate as u64,
            eta_secs: eta,
        });
        self.last_emit = now;
        self.rate_bytes_last = current_done;
        self.rate_time_last = now;
    }
}

/// 发送一组文件（一个任务）。阻塞至任务结束。
pub async fn send_files(
    session: Arc<Session>,
    task_id: u64,
    task_uid: [u8; 16],
    items: Vec<TransferItem>,
    sink: EventSink,
    cancel: Arc<AtomicBool>,
) -> BtResult<SendSummary> {
    let total_size: u64 = items.iter().map(|i| i.size).sum();
    let file_count = items.len() as u32;

    // 注册取消令牌（对端 CANCEL 会置位）
    session
        .task_cancels
        .lock()
        .unwrap()
        .insert(task_id, cancel.clone());

    sink(EngineEvent::State {
        conn_id: session.id,
        task_id,
        incoming: false,
        state: "waiting_accept".into(),
    });

    // 1) TRANSFER_REQ，等待接收方用户决定（携带全局唯一 task_uid）
    session.send_control(
        task_id,
        Message::TransferReq {
            task_uid,
            file_count,
            total_size,
            sender_name: session.identity.device_name(),
        },
    )?;
    let resp_rx = session.wait_resp(WaitKey::TransferResp(task_id));
    let resp = tokio::time::timeout(Duration::from_secs(600), resp_rx)
        .await
        .map_err(|_| BtError::ConnectTimeout)?
        .map_err(|_| BtError::ConnectTimeout)?;
    match resp {
        Message::TransferResp { accept: true, .. } => {}
        _ => {
            sink(EngineEvent::State {
                conn_id: session.id,
                task_id,
                incoming: false,
                state: TASK_STATE_REJECTED.into(),
            });
            return Ok(SendSummary { ok: 0, failed: 0 });
        }
    }

    if cancel.load(Ordering::SeqCst) {
        return Err(BtError::Cancelled);
    }

    sink(EngineEvent::State {
        conn_id: session.id,
        task_id,
        incoming: false,
        state: TASK_STATE_TRANSFERRING.into(),
    });

    // 2) 逐文件发送（并发受信号量约束）
    let task_start = Instant::now();
    let sem = Arc::new(tokio::sync::Semaphore::new(session.cfg.concurrency.max(1)));
    let ok_count = Arc::new(AtomicU32::new(0));
    let fail_count = Arc::new(AtomicU32::new(0));
    let cancel_sent = Arc::new(AtomicBool::new(false));
    let chunk_size = session.cfg.chunk_size;
    let tracker = Arc::new(std::sync::Mutex::new(TaskSendTracker::new(total_size, items.len())));

    let mut handles = Vec::new();
    for (idx, item) in items.into_iter().enumerate() {
        let session = session.clone();
        let sink = sink.clone();
        let cancel = cancel.clone();
        let sem = sem.clone();
        let ok_count = ok_count.clone();
        let fail_count = fail_count.clone();
        let cancel_sent = cancel_sent.clone();
        let tracker = tracker.clone();
        let file_seq = idx as u32;

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.ok();
            if cancel.load(Ordering::SeqCst) {
                fail_count.fetch_add(1, Ordering::SeqCst);
                return;
            }
            let res = send_one_file(
                session.clone(),
                task_id,
                file_seq,
                &item,
                chunk_size,
                &cancel,
                &sink,
                tracker,
            )
            .await;
            match res {
                Ok(true) => {
                    ok_count.fetch_add(1, Ordering::SeqCst);
                }
                Ok(false) => {
                    fail_count.fetch_add(1, Ordering::SeqCst);
                }
                Err(e) => {
                    if e == BtError::Cancelled {
                        if !cancel_sent.swap(true, Ordering::SeqCst) {
                            let _ = session.send_control(task_id, Message::Cancel { reason: CANCEL_REASON_USER });
                        }
                    } else {
                        sink(EngineEvent::Error {
                            conn_id: session.id,
                            task_id: Some(task_id),
                            incoming: false,
                            code: e.code(),
                            message: format!("{} 发送失败：{e}", item.rel_path),
                        });
                    }
                    fail_count.fetch_add(1, Ordering::SeqCst);
                }
            }
        }));
    }
    for h in handles {
        let _ = h.await;
    }

    let ok = ok_count.load(Ordering::SeqCst);
    let failed = fail_count.load(Ordering::SeqCst);
    let ms = task_start.elapsed().as_millis() as u64;
    let mbs = if ms > 0 {
        (total_size as f64 / 1048576.0) / (ms as f64 / 1000.0)
    } else {
        0.0
    };
    tracing::info!(
        task_id,
        files = file_count,
        bytes = total_size,
        ok,
        failed,
        ms,
        mbs,
        "send task finished"
    );
    session.task_cancels.lock().unwrap().remove(&task_id);
    if cancel.load(Ordering::SeqCst) {
        // 【核心修复】：退出前从 TaskSendTracker 获取全任务最终对齐确认的已完成字节数，
        // 统一向外发射终态 Progress 事件，杜绝发送端任务状态残留或与接收端数值脱节！
        let final_done = tracker.lock().unwrap().current_done();
        sink(EngineEvent::Progress {
            conn_id: session.id,
            task_id,
            incoming: false,
            rel_path: None,
            done: final_done,
            total: total_size,
            rate_bps: 0,
            eta_secs: 0,
        });
        return Err(BtError::Cancelled);
    }
    sink(EngineEvent::Summary {
        conn_id: session.id,
        task_id,
        incoming: false,
        ok,
        failed,
    });
    Ok(SendSummary { ok, failed })
}

/// 发送单个文件。Ok(true)=成功，Ok(false)=对端判失败，Err=异常。
async fn send_one_file(
    session: Arc<Session>,
    task_id: u64,
    file_seq: u32,
    item: &TransferItem,
    chunk_size: usize,
    cancel: &AtomicBool,
    sink: &EventSink,
    tracker: Arc<std::sync::Mutex<TaskSendTracker>>,
) -> BtResult<bool> {
    // 打开文件管道；无论成败（含中途任何 `?` 提前返回）都必须释放该管道：
    // 写句柄移除后写任务 FIN，QUIC 流配额才能归还（否则累计 64 个文件后
    // open_bi 因配额耗尽永久挂起）
    let pipe_id = session.open_file_pipe().await?;
    let res = send_one_file_on_pipe(
        &session, pipe_id, task_id, file_seq, item, chunk_size, cancel, sink, tracker,
    )
    .await;
    session.close_file_pipe(pipe_id);
    res
}

/// 在已打开的文件管道上发送单个文件。
#[allow(clippy::too_many_arguments)]
async fn send_one_file_on_pipe(
    session: &Arc<Session>,
    pipe_id: u64,
    task_id: u64,
    file_seq: u32,
    item: &TransferItem,
    chunk_size: usize,
    cancel: &AtomicBool,
    sink: &EventSink,
    tracker: Arc<std::sync::Mutex<TaskSendTracker>>,
) -> BtResult<bool> {
    let file_start = Instant::now();
    // 1) 发送文件元数据帧（标准向前兼容格式）
    // 包含文件序号、大小、修改时间戳、相对路径以及推荐分片大小与校验算法
    session.send_file(
        pipe_id,
        task_id,
        Message::FileMeta {
            file_seq,
            size: item.size,
            mtime: item.mtime_unix,
            rel_path: item.rel_path.clone(),
            chunk_size: chunk_size as u32,
            hash_algo: HASH_BLAKE3,
        },
    )?;

    let meta_rx = session.wait_resp(WaitKey::MetaAck {
        task_session: task_id,
        file_seq,
    });
    let meta_ack = tokio::time::timeout(TIMEOUT_FILE_META_ACK, meta_rx)
        .await
        .map_err(|_| BtError::ConnectTimeout)?
        .map_err(|_| BtError::ConnectTimeout)?;
    let Message::FileMetaAck {
        accept: true,
        ..
    } = meta_ack
    else {
        return Ok(false); // 对端拒收该文件
    };

    // 全任务进度发射（驱动 TaskSendTracker，全任务总大小与已完成大小始终对齐）
    let emit_progress = |file_done: u64, force: bool| {
        tracker.lock().unwrap().update_file(
            file_seq as usize,
            file_done,
            &item.rel_path,
            session.id,
            task_id,
            sink,
            force,
        );
    };

    // 打开读取器（边读边算）
    let mut reader = FileReader::open(&item.abs_path)?;
    let mut ack_rx = session.subscribe_acks(task_id, file_seq);
    let mut acked = 0u64;
    let mut offset = 0u64;

    // 在途（已发未确认）字节窗口：使用集中定义的常量，与底层 QUIC 流控窗口完全匹配
    const IN_FLIGHT_WINDOW: u64 = IN_FLIGHT_WINDOW_BYTES;

    loop {
        if cancel.load(Ordering::SeqCst) {
            session
                .ack_subs
                .lock()
                .unwrap()
                .remove(&(task_id, file_seq));
            return Err(BtError::Cancelled);
        }
        if session.is_closed() {
            session
                .ack_subs
                .lock()
                .unwrap()
                .remove(&(task_id, file_seq));
            return Err(BtError::ConnectTimeout);
        }

        // 背压：在途字节超窗口时先吸收 ACK 再继续，等待期间照常发进度
        loop {
            // 先尝试非阻塞清空吸收所有已到达的 ACK
            while let Ok(v) = ack_rx.try_recv() {
                acked = acked.max(v);
            }
            if offset.saturating_sub(acked) < IN_FLIGHT_WINDOW {
                break;
            }
            if session.is_closed() {
                session
                    .ack_subs
                    .lock()
                    .unwrap()
                    .remove(&(task_id, file_seq));
                return Err(BtError::ConnectTimeout);
            }
            match tokio::time::timeout(TIMEOUT_BACKPRESSURE_ACK, ack_rx.recv()).await {
                Ok(Some(v)) => {
                    acked = acked.max(v);
                    while let Ok(extra) = ack_rx.try_recv() {
                        acked = acked.max(extra);
                    }
                }
                Ok(None) => {
                    // ACK 订阅断开（会话已关闭）
                    session
                        .ack_subs
                        .lock()
                        .unwrap()
                        .remove(&(task_id, file_seq));
                    return Err(BtError::ConnectTimeout);
                }
                Err(_) => {} // 超时：回到循环顶部检查取消与断开
            }
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            if session.is_closed() {
                session
                    .ack_subs
                    .lock()
                    .unwrap()
                    .remove(&(task_id, file_seq));
                return Err(BtError::ConnectTimeout);
            }
            emit_progress(acked.min(offset), false);
        }

        if cancel.load(Ordering::SeqCst) {
            continue;
        }

        let Some(chunk) = reader.next_chunk(chunk_size)? else {
            break;
        };
        let len = chunk.len() as u64;
        session.send_file(
            pipe_id,
            task_id,
            Message::Data {
                file_seq,
                chunk_seq: offset / chunk_size as u64,
                payload: bytes::Bytes::copy_from_slice(chunk),
            },
        )?;
        offset += len;

        // 吸收累积确认
        while let Ok(v) = ack_rx.try_recv() {
            acked = acked.max(v);
        }
        emit_progress(acked.min(offset), false);
    }

    // FILE_DONE：整文件哈希比对
    let hash = reader.finalize();
    session.send_file(pipe_id, task_id, Message::FileDone { file_seq, hash })?;
    let done_rx = session.wait_resp(WaitKey::DoneAck {
        task_session: task_id,
        file_seq,
    });
    tokio::pin!(done_rx);

    // 当发送端读取完毕跳出循环时，底层 QUIC 管道与网络在途仍有在途分片。
    // 在等待接收端校验落盘并回传 DoneAck 的过程中，持续吸收对端回传的 ACK 并调用 emit_progress 平滑推进进度。
    let deadline = tokio::time::Instant::now() + TIMEOUT_FILE_DONE_ACK;
    let done_ack = loop {
        tokio::select! {
            res = &mut done_rx => {
                match res {
                    Ok(msg) => break msg,
                    Err(_) => return Err(BtError::ConnectTimeout),
                }
            }
            v = ack_rx.recv() => {
                match v {
                    Some(v) => {
                        acked = acked.max(v);
                        while let Ok(extra) = ack_rx.try_recv() {
                            acked = acked.max(extra);
                        }
                        emit_progress(acked.min(offset), false);
                    }
                    None => {
                        session
                            .ack_subs
                            .lock()
                            .unwrap()
                            .remove(&(task_id, file_seq));
                        return Err(BtError::ConnectTimeout);
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                while let Ok(v) = ack_rx.try_recv() {
                    acked = acked.max(v);
                }
                emit_progress(acked.min(offset), false);
                if cancel.load(Ordering::SeqCst) {
                    session
                        .ack_subs
                        .lock()
                        .unwrap()
                        .remove(&(task_id, file_seq));
                    return Err(BtError::Cancelled);
                }
                if session.is_closed() {
                    session
                        .ack_subs
                        .lock()
                        .unwrap()
                        .remove(&(task_id, file_seq));
                    return Err(BtError::ConnectTimeout);
                }
                if tokio::time::Instant::now() > deadline {
                    session
                        .ack_subs
                        .lock()
                        .unwrap()
                        .remove(&(task_id, file_seq));
                    return Err(BtError::ConnectTimeout);
                }
            }
        }
    };
    session
        .ack_subs
        .lock()
        .unwrap()
        .remove(&(task_id, file_seq));

    let ok = matches!(done_ack, Message::FileDoneAck { ok: true, .. });
    if ok {
        emit_progress(item.size, true);
    }
    let ms = file_start.elapsed().as_millis() as u64;
    let mbs = if ms > 0 {
        (item.size as f64 / 1048576.0) / (ms as f64 / 1000.0)
    } else {
        0.0
    };
    tracing::info!(
        task_id,
        file_seq,
        rel_path = %item.rel_path,
        size = item.size,
        ok,
        ms,
        mbs,
        "file sent"
    );
    sink(EngineEvent::FileFinished {
        conn_id: session.id,
        task_id,
        incoming: false,
        rel_path: item.rel_path.clone(),
        ok,
    });
    Ok(ok)
}
