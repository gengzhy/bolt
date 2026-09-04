//! 发送流水线（实施方案 7.2/7.3/8.1）。
//!
//! TRANSFER_REQ → 逐文件 FILE_META（带回断点区间协商）→ DATA 分片
//! （边读边算 BLAKE3，已收分片跳过）→ FILE_DONE 哈希比对。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lt_file::ranges::RangeSet;
use lt_file::reader::FileReader;
use lt_file::traverse::TransferItem;
use lt_utils::{LtError, LtResult};

use crate::protocol::{Message, HASH_BLAKE3};
use crate::session::{EngineEvent, EventSink, Session, WaitKey};

/// 发送汇总。
#[derive(Debug, Clone, Copy)]
pub struct SendSummary {
    pub ok: u32,
    pub failed: u32,
}

/// 发送一组文件（一个任务）。阻塞至任务结束。
pub async fn send_files(
    session: Arc<Session>,
    task_id: u64,
    task_uid: [u8; 16],
    items: Vec<TransferItem>,
    sink: EventSink,
    cancel: Arc<AtomicBool>,
) -> LtResult<SendSummary> {
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
        .map_err(|_| LtError::ConnectTimeout)?
        .map_err(|_| LtError::ConnectTimeout)?;
    match resp {
        Message::TransferResp { accept: true, .. } => {}
        _ => {
            sink(EngineEvent::State {
                conn_id: session.id,
                task_id,
                incoming: false,
                state: "rejected".into(),
            });
            return Err(LtError::TransferRejected);
        }
    }

    if cancel.load(Ordering::SeqCst) {
        return Err(LtError::Cancelled);
    }

    sink(EngineEvent::State {
        conn_id: session.id,
        task_id,
        incoming: false,
        state: "transferring".into(),
    });

    // 2) 逐文件发送（并发受信号量约束）
    let task_start = Instant::now();
    let sem = Arc::new(tokio::sync::Semaphore::new(session.cfg.concurrency.max(1)));
    let ok_count = Arc::new(AtomicU32::new(0));
    let fail_count = Arc::new(AtomicU32::new(0));
    let cancel_sent = Arc::new(AtomicBool::new(false));
    let chunk_size = session.cfg.chunk_size;

    let mut handles = Vec::new();
    for (idx, item) in items.into_iter().enumerate() {
        let session = session.clone();
        let sink = sink.clone();
        let cancel = cancel.clone();
        let sem = sem.clone();
        let ok_count = ok_count.clone();
        let fail_count = fail_count.clone();
        let cancel_sent = cancel_sent.clone();
        let file_seq = idx as u32;

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.ok();
            if cancel.load(Ordering::SeqCst) {
                fail_count.fetch_add(1, Ordering::SeqCst);
                return;
            }
            match send_one_file(
                session.clone(),
                task_id,
                file_seq,
                &item,
                chunk_size,
                &cancel,
                &sink,
            )
            .await
            {
                Ok(true) => {
                    ok_count.fetch_add(1, Ordering::SeqCst);
                }
                Ok(false) => {
                    fail_count.fetch_add(1, Ordering::SeqCst);
                }
                Err(e) => {
                    if e == LtError::Cancelled {
                        if !cancel_sent.swap(true, Ordering::SeqCst) {
                            let _ = session.send_control(task_id, Message::Cancel { reason: 9 });
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
        return Err(LtError::Cancelled);
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
) -> LtResult<bool> {
    // 打开文件管道；无论成败（含中途任何 `?` 提前返回）都必须释放该管道：
    // 写句柄移除后写任务 FIN，QUIC 流配额才能归还（否则累计 64 个文件后
    // open_bi 因配额耗尽永久挂起）
    let pipe_id = session.open_file_pipe().await?;
    let res = send_one_file_on_pipe(
        &session, pipe_id, task_id, file_seq, item, chunk_size, cancel, sink,
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
) -> LtResult<bool> {
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
    let meta_ack = tokio::time::timeout(Duration::from_secs(30), meta_rx)
        .await
        .map_err(|_| LtError::ConnectTimeout)?
        .map_err(|_| LtError::ConnectTimeout)?;
    let Message::FileMetaAck {
        accept: true,
        ranges,
        ..
    } = meta_ack
    else {
        return Ok(false); // 对端拒收该文件
    };
    let covered = RangeSet::from_intervals(ranges);

    // 打开读取器（边读边算）
    let mut reader = FileReader::open(&item.abs_path)?;
    let mut ack_rx = session.subscribe_acks(task_id, file_seq);
    let mut acked = covered.contiguous_prefix();
    let mut covered_bytes = covered.total_received();
    let mut offset = 0u64;
    let mut last_emit = Instant::now()
        .checked_sub(Duration::from_secs(1))
        .unwrap_or_else(Instant::now);
    // 断点续传时 acked 前缀可能很大：速率只统计本次新增字节
    let mut rate_last = acked;
    let mut rate_time = Instant::now();

    // 进度发射（250ms 节流，含速率/剩余时间计算）
    let mut emit_progress = move |done: u64| {
        let now = Instant::now();
        if now.duration_since(last_emit) < Duration::from_millis(250) {
            return;
        }
        let dt = now.duration_since(rate_time).as_secs_f64().max(0.001);
        let rate = (done.saturating_sub(rate_last)) as f64 / dt;
        let remaining = item.size.saturating_sub(done);
        let eta = if rate > 1.0 {
            (remaining as f64 / rate) as u64
        } else {
            0
        };
        sink(EngineEvent::Progress {
            conn_id: session.id,
            task_id,
            incoming: false,
            rel_path: Some(item.rel_path.clone()),
            done,
            total: item.size,
            rate_bps: rate as u64,
            eta_secs: eta,
        });
        last_emit = now;
        rate_last = done;
        rate_time = now;
    };

    // 若存在断点已收前缀，立刻触发一次初始进度事件，使发送端 UI 与对端立即对齐
    if covered_bytes > 0 {
        sink(EngineEvent::Progress {
            conn_id: session.id,
            task_id,
            incoming: false,
            rel_path: Some(item.rel_path.clone()),
            done: acked.max(covered_bytes),
            total: item.size,
            rate_bps: 0,
            eta_secs: 0,
        });
    }

    // 在途（已发未确认）字节窗口。文件管道队列无背压，若不限制，读循环会在
    // 极短时间内把整个文件读进内存队列，进度门只在开头触发一次（UI 卡 0%），
    // 且大文件会撑爆内存。局域网调优扩大到 32MB，配合接收端 4MB 即时 ACK，
    // 流水线始终处于饱满传输状态且绝无饥饿停等。
    const IN_FLIGHT_WINDOW: u64 = 32 * 1024 * 1024;

    loop {
        if cancel.load(Ordering::SeqCst) {
            session
                .ack_subs
                .lock()
                .unwrap()
                .remove(&(task_id, file_seq));
            return Err(LtError::Cancelled);
        }

        // 背压：在途字节超窗口时先吸收 ACK 再继续，等待期间照常发进度
        loop {
            let done_now = acked.max(covered_bytes.min(offset));
            if offset.saturating_sub(done_now) < IN_FLIGHT_WINDOW {
                break;
            }
            match tokio::time::timeout(Duration::from_millis(500), ack_rx.recv()).await {
                Ok(Some(v)) => acked = acked.max(v),
                Ok(None) => {
                    // ACK 订阅断开（会话已关闭）
                    session
                        .ack_subs
                        .lock()
                        .unwrap()
                        .remove(&(task_id, file_seq));
                    return Err(LtError::ConnectTimeout);
                }
                Err(_) => {} // 超时：回到循环顶部检查取消
            }
            if cancel.load(Ordering::SeqCst) {
                session
                    .ack_subs
                    .lock()
                    .unwrap()
                    .remove(&(task_id, file_seq));
                return Err(LtError::Cancelled);
            }
            emit_progress(acked.max(covered_bytes.min(offset)));
        }

        let Some(chunk) = reader.next_chunk(chunk_size)? else {
            break;
        };
        let len = chunk.len() as u64;
        if covered.covers(offset, len) {
            covered_bytes = covered_bytes.max(offset + len);
        } else {
            session.send_file(
                pipe_id,
                task_id,
                Message::Data {
                    file_seq,
                    chunk_seq: offset / chunk_size as u64,
                    payload: bytes::Bytes::copy_from_slice(chunk),
                },
            )?;
        }
        offset += len;

        // 吸收累积确认
        while let Ok(v) = ack_rx.try_recv() {
            acked = acked.max(v);
        }
        emit_progress(acked.max(covered_bytes.min(offset)));
    }

    // FILE_DONE：整文件哈希比对
    let hash = reader.finalize();
    session.send_file(pipe_id, task_id, Message::FileDone { file_seq, hash })?;
    let done_rx = session.wait_resp(WaitKey::DoneAck {
        task_session: task_id,
        file_seq,
    });
    let done_ack = tokio::select! {
        res = tokio::time::timeout(Duration::from_secs(300), done_rx) => {
            res.map_err(|_| LtError::ConnectTimeout)?.map_err(|_| LtError::ConnectTimeout)?
        }
        _ = async {
            loop {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        } => {
            return Err(LtError::Cancelled);
        }
    };
    session
        .ack_subs
        .lock()
        .unwrap()
        .remove(&(task_id, file_seq));

    let ok = matches!(done_ack, Message::FileDoneAck { ok: true, .. });
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
