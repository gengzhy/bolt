//! 会话状态机（实施方案 4.3/5.2/7.1）。
//!
//! 每条连接一个 [`Session`]：HELLO 握手 → 信任校验（TOFU）→ 必要时配对，
//! 之后承载双向的传输请求/文件流/心跳。
//!
//! 架构：所有管道（控制流 + 文件流）的读任务把帧泵入单一调度器
//! （[`dispatcher`]），由调度器按指令码路由；写方向通过各管道的
//! mpsc 队列串行化，天然避免帧交错损坏。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use std::net::TcpStream;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

use crypto::trust::{TrustStatus, TrustStore};
use crypto::{pairing, DeviceIdentity};
use utils::constants::*;
use utils::{BtError, BtResult, PROTOCOL_VERSION};

use crate::conn::{self, Incoming, Pipe};
use crate::protocol::{DeviceType, Message};
use crate::quic;

/// 传输通道类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Quic,
    Tcp,
}

/// 已建立的会话信息。
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub conn_id: u64,
    pub peer_uuid: String,
    pub peer_name: String,
    pub peer_type: DeviceType,
    pub peer_fingerprint: String,
    pub transport: TransportKind,
    pub addr: String,
}

/// 入站传输请求（交由上层用户决定）。
#[derive(Debug, Clone)]
pub struct IncomingTransfer {
    /// 全局唯一的 128 位任务 ID（UUID）
    pub task_uid: [u8; 16],
    pub req_id: u64,
    pub conn_id: u64,
    pub sender_uuid: String,
    pub sender_name: String,
    pub file_count: u32,
    pub total_size: u64,
}

/// 引擎向上层推送的事件（进度/状态/错误）。
#[derive(Debug, Clone)]
pub enum EngineEvent {
    State {
        /// 事件所属会话的 conn_id：上层按 (conn_id, 线上任务号) 翻译入站任务号，
        /// 避免多设备同时发送时对端任务号撞车（两端任务号都从 1 递增）
        conn_id: u64,
        task_id: u64,
        incoming: bool,
        state: String,
    },
    Progress {
        conn_id: u64,
        task_id: u64,
        incoming: bool,
        rel_path: Option<String>,
        done: u64,
        total: u64,
        rate_bps: u64,
        eta_secs: u64,
    },
    FileFinished {
        conn_id: u64,
        task_id: u64,
        incoming: bool,
        rel_path: String,
        ok: bool,
    },
    Summary {
        conn_id: u64,
        task_id: u64,
        incoming: bool,
        ok: u32,
        failed: u32,
    },
    Error {
        conn_id: u64,
        task_id: Option<u64>,
        /// true = 接收（入站）任务事件。入站任务的 task_id 为对端线上任务号
        /// （wire id），上层需映射为本地任务号后再更新记录。
        incoming: bool,
        code: i32,
        message: String,
    },
}

pub type EventSink = Arc<dyn Fn(EngineEvent) + Send + Sync>;

/// 会话级配置（由上层从 AppConfig 装配）。
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub chunk_size: usize,
    pub concurrency: usize,
    pub save_dir: PathBuf,
    pub tmp_dir: PathBuf,
    /// 断点缓存/信任库所在的应用私有目录
    pub data_dir: PathBuf,
    pub collision: file::writer::NameCollisionPolicy,
    pub device_type: DeviceType,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            chunk_size: utils::constants::DEFAULT_CHUNK_SIZE,
            concurrency: 4,
            save_dir: PathBuf::from("received"),
            tmp_dir: PathBuf::from("tmp"),
            data_dir: PathBuf::from("."),
            collision: file::writer::NameCollisionPolicy::AutoRename,
            device_type: DeviceType::Unknown,
        }
    }
}

/// 上层回调（task / cli 实现）。
pub trait SessionHandler: Send + Sync {
    /// 握手（含配对）完成。
    fn connected(&self, _session: Arc<Session>, _info: SessionInfo) {}
    /// 需要配对：展示验证码，用户确认后调用 [`Session::respond_pair`]。
    fn pair_needed(
        &self,
        _session: Arc<Session>,
        _pair_id: u64,
        _info: SessionInfo,
        _code: String,
        _is_initiator: bool,
    ) {
    }
    /// 入站传输请求：用户决定后调用 [`Session::respond_transfer`]。
    fn transfer_incoming(&self, _session: Arc<Session>, _req: IncomingTransfer) {}
    /// 会话断开。
    fn disconnected(&self, _conn_id: u64, _err_code: Option<i32>) {}
    /// 引擎事件（任务状态/进度/错误）。
    fn event(&self, _ev: EngineEvent) {}
}

/// 等待应答的消息键。
#[derive(Hash, Eq, PartialEq, Clone)]
pub enum WaitKey {
    TransferResp(u64),
    PairResp,
    /// 文件元数据应答/文件完成应答：必须带任务号——同一会话上并发跑多个
    /// 任务时，两侧任务的 file_seq 都从 0 递增，仅按 file_seq 索引会把
    /// 一个任务的应答错发给另一个任务（挂死 30s 或结果串扰）。
    MetaAck {
        task_session: u64,
        file_seq: u32,
    },
    DoneAck {
        task_session: u64,
        file_seq: u32,
    },
}

pub(crate) enum Disp {
    Frame {
        pipe_id: u64,
        task_session: u64,
        frame: Result<Message, crate::protocol::CodecError>,
    },
    StartRecv {
        task_session: u64,
        req: IncomingTransfer,
    },
    FileVerified {
        task_session: u64,
        file_seq: u32,
        result: BtResult<PathBuf>,
        rel_path: String,
    },
    /// 管道读循环结束（EOF/传输错误）。控制管道（0）下来 = 对端断开，
    /// 需立即关闭会话，不等 90s 心跳；文件管道下来则清理写句柄、归还流配额。
    TransportDown { pipe_id: u64 },
}

/// 供 recv 模块构造 FileVerified 调度消息。
pub(crate) fn disp_file_verified(
    task_session: u64,
    file_seq: u32,
    result: BtResult<PathBuf>,
    rel_path: String,
) -> Disp {
    Disp::FileVerified {
        task_session,
        file_seq,
        result,
        rel_path,
    }
}

/// 一条已建立的设备间会话。
pub struct Session {
    pub id: u64,
    kind: TransportKind,
    pub(crate) handler: Arc<dyn SessionHandler>,
    pub(crate) identity: Arc<DeviceIdentity>,
    pub(crate) trust: Arc<TrustStore>,
    pub cfg: SessionConfig,
    control_tx: mpsc::UnboundedSender<(u64, Message)>,
    pub(crate) disp_tx: mpsc::UnboundedSender<Disp>,
    pipe_txs: Mutex<HashMap<u64, mpsc::UnboundedSender<(u64, Message)>>>,
    next_pipe_id: AtomicU64,
    quic_conn: Option<quinn::Connection>,
    /// QUIC 客户端 Endpoint 必须随会话存活（quinn 的 EndpointDriver
    /// 在最后一个 Endpoint 句柄被 drop 时终止，连接随之失效）。
    _quic_ep: Option<quinn::Endpoint>,
    /// TCP 原始流克隆（kill 句柄）：close 时 shutdown，让对端立即感知。
    tcp_kill: Option<Arc<TcpStream>>,
    pair_decision: Mutex<Option<oneshot::Sender<bool>>>,
    transfer_decisions: Mutex<HashMap<u64, oneshot::Sender<bool>>>,
    resp_waiters: Mutex<HashMap<WaitKey, oneshot::Sender<Message>>>,
    pub(crate) ack_subs: Mutex<HashMap<(u64, u32), mpsc::UnboundedSender<u64>>>,
    pub(crate) task_cancels: Mutex<HashMap<u64, Arc<AtomicBool>>>,
    last_seen_ms: AtomicU64,
    closed: Arc<AtomicBool>,
    info: Mutex<SessionInfo>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Session {
    pub fn info(&self) -> SessionInfo {
        self.info.lock().unwrap().clone()
    }

    pub fn transport(&self) -> TransportKind {
        self.kind
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// 控制面发送（会话字段为 0 或任务ID）。
    pub fn send_control(&self, task_session: u64, msg: Message) -> BtResult<()> {
        self.control_tx
            .send((task_session, msg))
            .map_err(|_| BtError::ConnectTimeout)
    }

    /// 文件面发送（pipe_id 由 [`Session::open_file_pipe`] 给出）。
    pub fn send_file(&self, pipe_id: u64, task_session: u64, msg: Message) -> BtResult<()> {
        let txs = self.pipe_txs.lock().unwrap();
        let tx = txs.get(&pipe_id).ok_or(BtError::ConnectTimeout)?;
        tx.send((task_session, msg))
            .map_err(|_| BtError::ConnectTimeout)
    }

    /// 打开一条文件管道（QUIC 新建双向流；TCP 复用控制连接）。
    pub async fn open_file_pipe(&self) -> BtResult<u64> {
        match self.kind {
            TransportKind::Tcp => Ok(0),
            TransportKind::Quic => {
                let conn = self.quic_conn.as_ref().ok_or(BtError::ConnectTimeout)?;
                // 带超时：对端流配额耗尽时 open_bi 会无限等待
                let (send, recv) = timeout(Duration::from_secs(30), conn.open_bi())
                    .await
                    .map_err(|_| BtError::ConnectTimeout)?
                    .map_err(|_| BtError::ConnectTimeout)?;
                Ok(self.register_pipe(quic::pipe_from_bi(send, recv)))
            }
        }
    }

    /// 关闭一条文件管道（发送端在单文件结束后调用）：移除写句柄，
    /// 写任务排空队列后 FIN，双向终结后 QUIC 流配额归还。
    /// TCP 复用控制连接（pipe 0），无需处理。
    pub fn close_file_pipe(&self, pipe_id: u64) {
        if pipe_id == 0 {
            return;
        }
        self.pipe_txs.lock().unwrap().remove(&pipe_id);
    }

    /// 注册一条管道：启动读/写任务，返回 pipe_id。
    pub(crate) fn register_pipe(&self, pipe: Pipe) -> u64 {
        let pipe_id = self.next_pipe_id.fetch_add(1, Ordering::SeqCst);
        let (tx, mut rx) = mpsc::unbounded_channel::<(u64, Message)>();
        self.pipe_txs.lock().unwrap().insert(pipe_id, tx.clone());
        // 写任务：队列排空后优雅 FIN（quinn SendStream 的 shutdown 即 finish）。
        // 注意：文件管道的存亡不影响会话——绝不能在此置位会话级 closed，
        // 否则任一文件流异常都会杀死调度器，后续发送全部 -3 超时。
        let mut writer = pipe.writer;
        tokio::spawn(async move {
            while let Some((sess, msg)) = rx.recv().await {
                if let Err(e) = conn::write_frame(&mut writer, sess, &msg).await {
                    tracing::error!(pipe_id, error = ?e, "file pipe write_frame error");
                    break;
                }
            }
            let _ = writer.shutdown().await;
        });
        let mut reader = pipe.reader;
        let disp_tx = self.disp_tx.clone();
        tokio::spawn(async move {
            loop {
                match conn::read_frame(&mut reader).await {
                    Ok(Some(Incoming::Msg { session, msg })) => {
                        if disp_tx
                            .send(Disp::Frame {
                                pipe_id,
                                task_session: session,
                                frame: Ok(msg),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(Some(Incoming::Bad { session, err })) => {
                        if disp_tx
                            .send(Disp::Frame {
                                pipe_id,
                                task_session: session,
                                frame: Err(err),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(None) => {
                        tracing::debug!(pipe_id, "file pipe read_frame returned EOF");
                        break;
                    }
                    Err(e) => {
                        tracing::error!(pipe_id, error = ?e, "file pipe read_frame returned Err");
                        break;
                    }
                }
            }
            // 读循环结束 = 管道死亡；控制管道（0）死亡即对端断开
            let _ = disp_tx.send(Disp::TransportDown { pipe_id });
        });
        pipe_id
    }

    /// 注册任务取消令牌（对端 CANCEL 或本地取消时置位）。
    pub fn register_task_cancel(&self, task_id: u64) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.task_cancels
            .lock()
            .unwrap()
            .insert(task_id, flag.clone());
        flag
    }

    pub fn cancel_task(&self, task_id: u64, reason: u32) {
        if let Some(f) = self.task_cancels.lock().unwrap().get(&task_id) {
            f.store(true, Ordering::SeqCst);
        }
        // 1. 向网络对端发送控制帧
        let _ = self.send_control(task_id, Message::Cancel { reason });
        // 2. 【核心修复】：同时向本地调度器投递 Cancel 帧，确保本地接收任务（若存在）
        // 也能第一时间执行 on_cancel，将已安全写入的连续前缀落盘持久化到 resume_store！
        let _ = self.disp_tx.send(Disp::Frame {
            pipe_id: 0,
            task_session: task_id,
            frame: Ok(Message::Cancel { reason }),
        });
    }

    /// 等待某一应答消息（调度器收到后投递）。
    pub fn wait_resp(&self, key: WaitKey) -> oneshot::Receiver<Message> {
        let (tx, rx) = oneshot::channel();
        self.resp_waiters.lock().unwrap().insert(key, tx);
        rx
    }

    /// 订阅某文件的累积确认偏移（按 任务号+文件序号 索引，
    /// 并发多任务时互不串扰，见 [`WaitKey::MetaAck`] 的说明）。
    pub fn subscribe_acks(&self, task_session: u64, file_seq: u32) -> mpsc::UnboundedReceiver<u64> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.ack_subs
            .lock()
            .unwrap()
            .insert((task_session, file_seq), tx);
        rx
    }

    /// 配对决定（用户确认验证码后）。
    pub fn respond_pair(&self, _pair_id: u64, accept: bool) {
        if let Some(tx) = self.pair_decision.lock().unwrap().take() {
            let _ = tx.send(accept);
        }
    }

    /// 传输请求决定（接收方用户接受/拒绝）。
    pub fn respond_transfer(&self, req_id: u64, accept: bool) {
        if let Some(tx) = self.transfer_decisions.lock().unwrap().remove(&req_id) {
            let _ = tx.send(accept);
        }
    }

    /// 关闭会话。
    ///
    /// BYE 先入写队列，200ms 宽限 flush 后再切断传输；对端读循环随传输
    /// 死亡立即结束（TransportDown），无需等 90s 心跳。宽限线程用 std
    /// 线程——close 可能跑在非 tokio 上下文（FFI/UI 线程）。
    pub fn close(&self, err_code: Option<i32>) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = self.send_control(0, Message::Bye);
        if let Some(q) = self.quic_conn.clone() {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(200));
                q.close(0u32.into(), b"bye");
            });
        }
        if let Some(k) = self.tcp_kill.clone() {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(200));
                let _ = k.shutdown(std::net::Shutdown::Both);
            });
        }
        self.handler.disconnected(self.id, err_code);
    }

    pub(crate) fn emit(&self, ev: EngineEvent) {
        self.handler.event(ev);
    }

    fn resolve_waiter(&self, key: &WaitKey, msg: Message) {
        if let Some(tx) = self.resp_waiters.lock().unwrap().remove(key) {
            let _ = tx.send(msg);
        }
    }
}

// ---------------- 握手 ----------------

/// 握手上下文（由 engine 构造）。
pub(crate) struct HandshakeCtx {
    pub identity: Arc<DeviceIdentity>,
    pub trust: Arc<TrustStore>,
    pub handler: Arc<dyn SessionHandler>,
    pub cfg: SessionConfig,
    pub kind: TransportKind,
    pub conn_id: u64,
    pub addr: String,
    pub quic: Option<quinn::Connection>,
    pub quic_ep: Option<quinn::Endpoint>,
    pub tcp_kill: Option<Arc<TcpStream>>,
}

fn hello_msg(identity: &DeviceIdentity, device_type: DeviceType) -> Message {
    Message::Hello {
        proto_ver: PROTOCOL_VERSION,
        caps: 0,
        uuid: identity.uuid.clone(),
        device_name: identity.device_name(),
        device_type,
        fingerprint: identity.fingerprint.clone(),
    }
}

/// 出方向握手（主动拨号方）。
pub(crate) async fn handshake_out(ctx: HandshakeCtx, pipe: Pipe) -> BtResult<Arc<Session>> {
    let (mut reader, writer) = (pipe.reader, pipe.writer);

    let (control_tx, control_rx) = mpsc::unbounded_channel::<(u64, Message)>();
    let (disp_tx, disp_rx) = mpsc::unbounded_channel::<Disp>();
    let info_placeholder = SessionInfo {
        conn_id: ctx.conn_id,
        peer_uuid: String::new(),
        peer_name: String::new(),
        peer_type: DeviceType::Unknown,
        peer_fingerprint: String::new(),
        transport: ctx.kind,
        addr: ctx.addr.clone(),
    };

    let session = Arc::new(Session {
        id: ctx.conn_id,
        kind: ctx.kind,
        handler: ctx.handler.clone(),
        identity: ctx.identity.clone(),
        trust: ctx.trust.clone(),
        cfg: ctx.cfg.clone(),
        control_tx: control_tx.clone(),
        disp_tx: disp_tx.clone(),
        pipe_txs: Mutex::new(HashMap::new()),
        next_pipe_id: AtomicU64::new(1),
        quic_conn: ctx.quic.clone(),
        _quic_ep: ctx.quic_ep.clone(),
        tcp_kill: ctx.tcp_kill.clone(),
        pair_decision: Mutex::new(None),
        transfer_decisions: Mutex::new(HashMap::new()),
        resp_waiters: Mutex::new(HashMap::new()),
        ack_subs: Mutex::new(HashMap::new()),
        task_cancels: Mutex::new(HashMap::new()),
        last_seen_ms: AtomicU64::new(now_ms()),
        closed: Arc::new(AtomicBool::new(false)),
        info: Mutex::new(info_placeholder),
    });
    // 控制管道注册（pipe_id 0）
    session
        .pipe_txs
        .lock()
        .unwrap()
        .insert(0, control_tx.clone());
    spawn_writer(control_rx, writer, session.closed.clone());

    // 1) HELLO 交换（本端 HELLO 经控制队列发出）
    session.send_control(0, hello_msg(&ctx.identity, ctx.cfg.device_type))?;

    let hello = read_hello(&mut reader).await?;
    let Message::Hello {
        uuid,
        device_name,
        device_type,
        fingerprint,
        proto_ver,
        ..
    } = hello
    else {
        return Err(BtError::ProtocolIncompatible);
    };
    if uuid == ctx.identity.uuid {
        return Err(BtError::InvalidArgument); // 连到了自己
    }
    let _ = proto_ver;

    // 2) 信任校验 + 配对（出方向：发起配对并等待对方决定）
    match ctx.trust.check(&uuid, &fingerprint) {
        TrustStatus::Changed => {
            session.send_control(
                0,
                Message::ErrorMsg {
                    code: BtError::FingerprintChanged.code(),
                    detail: "certificate fingerprint changed".into(),
                },
            )?;
            return Err(BtError::FingerprintChanged);
        }
        TrustStatus::Unknown => {
            let mut nonce = [0u8; 16];
            rand_fill(&mut nonce);
            session.send_control(0, Message::PairReq { nonce })?;
            let code = pairing::verification_code(&ctx.identity.fingerprint, &fingerprint);
            let info = SessionInfo {
                conn_id: ctx.conn_id,
                peer_uuid: uuid.clone(),
                peer_name: device_name.clone(),
                peer_type: device_type,
                peer_fingerprint: fingerprint.clone(),
                transport: ctx.kind,
                addr: ctx.addr.clone(),
            };
            *session.info.lock().unwrap() = info.clone();
            let (dtx, mut drx) = oneshot::channel();
            *session.pair_decision.lock().unwrap() = Some(dtx);
            ctx.handler
                .pair_needed(session.clone(), ctx.conn_id, info, code, true);
            // 等待对端 PAIR_RESP 或发起方用户主动取消。
            // 注意：调度器尚未启动（finish_handshake 在配对后才执行），
            // 必须在此直接读控制流，否则应答帧无人读取 → 死等。
            let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
            let mut cancelled = false;
            let resp = loop {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    return Err(BtError::PairingFailed);
                }
                tokio::select! {
                    user_cancel = &mut drx, if !cancelled => {
                        match user_cancel {
                            Ok(false) => {
                                let _ = session.send_control(0, Message::Bye);
                                session.close(None);
                                return Err(BtError::Cancelled);
                            }
                            _ => {
                                cancelled = true;
                            }
                        }
                    }
                    frame = timeout(remaining, conn::read_frame(&mut reader)) => {
                        let frame = match frame {
                            Ok(r) => r,
                            Err(_) => return Err(BtError::PairingFailed),
                        };
                        let frame = match frame {
                            Ok(Some(f)) => f,
                            Ok(None) => return Err(BtError::PairingFailed), // EOF
                            Err(_) => return Err(BtError::PairingFailed),
                        };
                        match frame {
                            Incoming::Msg {
                                msg: msg @ Message::PairResp { .. },
                                ..
                            } => break msg,
                            Incoming::Msg {
                                msg: Message::Ping { seq },
                                ..
                            } => {
                                // 对端已进入心跳期，保持应答避免其超时断开
                                let _ = session.send_control(0, Message::Pong { seq });
                            }
                            Incoming::Msg {
                                msg: Message::PairReq { .. },
                                ..
                            } => {
                                // 对端同时发起配对（双向未知）：走迟到配对流程
                                handle_late_pair_req(&session);
                            }
                            _ => {} // 握手期其他杂帧忽略
                        }
                    }
                }
            };
            *session.pair_decision.lock().unwrap() = None;
            match resp {
                Message::PairResp { accept: true, .. } => {
                    ctx.trust.add(&uuid, &fingerprint, &device_name)?;
                }
                _ => {
                    session.close(None);
                    return Err(BtError::PairingFailed);
                }
            }
        }
        TrustStatus::Trusted => {}
    }

    let info = SessionInfo {
        conn_id: ctx.conn_id,
        peer_uuid: uuid,
        peer_name: device_name,
        peer_type: device_type,
        peer_fingerprint: fingerprint,
        transport: ctx.kind,
        addr: ctx.addr.clone(),
    };
    *session.info.lock().unwrap() = info.clone();

    tracing::info!(
        conn_id = session.id,
        uuid = %info.peer_uuid,
        transport = ?session.kind,
        "session established"
    );
    finish_handshake(session.clone(), reader, disp_tx, disp_rx);
    ctx.handler.connected(session.clone(), info);
    Ok(session)
}

/// 入方向握手（被连接方）。
pub(crate) async fn handshake_in(ctx: HandshakeCtx, pipe: Pipe) -> BtResult<Arc<Session>> {
    let (mut reader, writer) = (pipe.reader, pipe.writer);

    let (control_tx, control_rx) = mpsc::unbounded_channel::<(u64, Message)>();
    let (disp_tx, disp_rx) = mpsc::unbounded_channel::<Disp>();
    let info_placeholder = SessionInfo {
        conn_id: ctx.conn_id,
        peer_uuid: String::new(),
        peer_name: String::new(),
        peer_type: DeviceType::Unknown,
        peer_fingerprint: String::new(),
        transport: ctx.kind,
        addr: ctx.addr.clone(),
    };
    let session = Arc::new(Session {
        id: ctx.conn_id,
        kind: ctx.kind,
        handler: ctx.handler.clone(),
        identity: ctx.identity.clone(),
        trust: ctx.trust.clone(),
        cfg: ctx.cfg.clone(),
        control_tx: control_tx.clone(),
        disp_tx: disp_tx.clone(),
        pipe_txs: Mutex::new(HashMap::new()),
        next_pipe_id: AtomicU64::new(1),
        quic_conn: ctx.quic.clone(),
        _quic_ep: ctx.quic_ep.clone(),
        tcp_kill: ctx.tcp_kill.clone(),
        pair_decision: Mutex::new(None),
        transfer_decisions: Mutex::new(HashMap::new()),
        resp_waiters: Mutex::new(HashMap::new()),
        ack_subs: Mutex::new(HashMap::new()),
        task_cancels: Mutex::new(HashMap::new()),
        last_seen_ms: AtomicU64::new(now_ms()),
        closed: Arc::new(AtomicBool::new(false)),
        info: Mutex::new(info_placeholder),
    });
    session
        .pipe_txs
        .lock()
        .unwrap()
        .insert(0, control_tx.clone());
    spawn_writer(control_rx, writer, session.closed.clone());

    // 1) 先收对端 HELLO，再回 HELLO
    let hello = read_hello(&mut reader).await?;
    let Message::Hello {
        uuid,
        device_name,
        device_type,
        fingerprint,
        ..
    } = hello
    else {
        return Err(BtError::ProtocolIncompatible);
    };
    if uuid == ctx.identity.uuid {
        return Err(BtError::InvalidArgument);
    }
    session.send_control(0, hello_msg(&ctx.identity, ctx.cfg.device_type))?;

    // 2) 信任校验
    match ctx.trust.check(&uuid, &fingerprint) {
        TrustStatus::Changed => {
            session.send_control(
                0,
                Message::ErrorMsg {
                    code: BtError::FingerprintChanged.code(),
                    detail: "certificate fingerprint changed".into(),
                },
            )?;
            return Err(BtError::FingerprintChanged);
        }
        TrustStatus::Unknown => {
            // 等待发起方 PAIR_REQ（10s 内未到则由调度器处理迟到的配对）
            let frame = timeout(Duration::from_secs(10), conn::read_frame(&mut reader)).await;
            match frame {
                Ok(Ok(Some(Incoming::Msg {
                    msg: Message::PairReq { .. },
                    ..
                }))) => {
                    let code = pairing::verification_code(&ctx.identity.fingerprint, &fingerprint);
                    let info = SessionInfo {
                        conn_id: ctx.conn_id,
                        peer_uuid: uuid.clone(),
                        peer_name: device_name.clone(),
                        peer_type: device_type,
                        peer_fingerprint: fingerprint.clone(),
                        transport: ctx.kind,
                        addr: ctx.addr.clone(),
                    };
                    *session.info.lock().unwrap() = info.clone();
                    let (dtx, mut drx) = oneshot::channel();
                    *session.pair_decision.lock().unwrap() = Some(dtx);
                    ctx.handler
                        .pair_needed(session.clone(), ctx.conn_id, info, code, false);
                    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
                    let accept = loop {
                        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                        if remaining.is_zero() {
                            break false;
                        }
                        tokio::select! {
                            res = &mut drx => {
                                break res.unwrap_or(false);
                            }
                            frame = timeout(remaining, conn::read_frame(&mut reader)) => {
                                let frame = match frame {
                                    Ok(Ok(Some(f))) => f,
                                    _ => {
                                        session.close(None);
                                        return Err(BtError::Cancelled);
                                    }
                                };
                                match frame {
                                    Incoming::Msg { msg: Message::Bye, .. } => {
                                        session.close(None);
                                        return Err(BtError::Cancelled);
                                    }
                                    Incoming::Msg { msg: Message::Ping { seq }, .. } => {
                                        let _ = session.send_control(0, Message::Pong { seq });
                                    }
                                    _ => {}
                                }
                            }
                        }
                    };
                    *session.pair_decision.lock().unwrap() = None;
                    let mut nonce = [0u8; 16];
                    rand_fill(&mut nonce);
                    let _ = session.send_control(0, Message::PairResp { accept, nonce });
                    if accept {
                        ctx.trust.add(&uuid, &fingerprint, &device_name)?;
                    } else {
                        session.close(None);
                        return Err(BtError::PairingFailed);
                    }
                }
                _ => {
                    // 发起方未走配对（其信任库残留）：保持未知状态，
                    // 后续未配对设备的传输请求将被拒绝（-11）
                    tracing::debug!(%uuid, "peer did not initiate pairing");
                }
            }
        }
        TrustStatus::Trusted => {}
    }

    let info = SessionInfo {
        conn_id: ctx.conn_id,
        peer_uuid: uuid,
        peer_name: device_name,
        peer_type: device_type,
        peer_fingerprint: fingerprint,
        transport: ctx.kind,
        addr: ctx.addr.clone(),
    };
    *session.info.lock().unwrap() = info.clone();

    tracing::info!(
        conn_id = session.id,
        uuid = %info.peer_uuid,
        transport = ?session.kind,
        "session established"
    );
    finish_handshake(session.clone(), reader, disp_tx, disp_rx);
    ctx.handler.connected(session.clone(), info);
    Ok(session)
}

/// 握手收尾：移交读半部给管道0，启动调度器与心跳。
fn finish_handshake(
    session: Arc<Session>,
    reader: Box<dyn tokio::io::AsyncRead + Unpin + Send>,
    _disp_tx: mpsc::UnboundedSender<Disp>,
    disp_rx: mpsc::UnboundedReceiver<Disp>,
) {
    // 控制管道的读任务（把帧泵入调度器）
    let disp_tx = session.disp_tx.clone();
    let sess_pipe0 = session.clone();
    tokio::spawn(async move {
        let mut reader = reader;
        let conn_opt = sess_pipe0.quic_conn.clone();
        loop {
            match conn::read_frame(&mut reader).await {
                Ok(Some(Incoming::Msg { session: s, msg })) => {
                    if disp_tx
                        .send(Disp::Frame {
                            pipe_id: 0,
                            task_session: s,
                            frame: Ok(msg),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Some(Incoming::Bad { session: s, err })) => {
                    if disp_tx
                        .send(Disp::Frame {
                            pipe_id: 0,
                            task_session: s,
                            frame: Err(err),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(None) => {
                    let reason = conn_opt.as_ref().and_then(|c| c.close_reason());
                    tracing::warn!(conn_id = sess_pipe0.id, ?reason, "pipe 0 read_frame returned EOF");
                    break;
                }
                Err(e) => {
                    let reason = conn_opt.as_ref().and_then(|c| c.close_reason());
                    tracing::error!(conn_id = sess_pipe0.id, error = ?e, ?reason, "pipe 0 read_frame returned Err");
                    break;
                }
            }
        }
        // 控制管道读循环结束 = 传输死亡（或调度器已先行退出）：
        // 立即关闭会话，不等 90s 心跳
        let _ = disp_tx.send(Disp::TransportDown { pipe_id: 0 });
    });

    // QUIC：接受对端打开的后续文件流。拨号/被连两侧必须对称——
    // 拨号端若缺此循环，对端打开的文件流无人接收，反向发送必然超时（-3）
    if session.kind == TransportKind::Quic {
        if let Some(conn) = session.quic_conn.clone() {
            let sess = session.clone();
            tokio::spawn(async move {
                loop {
                    let Ok((send, recv)) = conn.accept_bi().await else {
                        break;
                    };
                    if sess.is_closed() {
                        break;
                    }
                    sess.register_pipe(quic::pipe_from_bi(send, recv));
                }
            });
        }
    }

    let s2 = session.clone();
    tokio::spawn(dispatcher(session.clone(), disp_rx));
    tokio::spawn(heartbeat(s2));
}

fn spawn_writer(
    mut rx: mpsc::UnboundedReceiver<(u64, Message)>,
    mut writer: Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
    closed: Arc<AtomicBool>,
) {
    tokio::spawn(async move {
        while let Some((sess, msg)) = rx.recv().await {
            if let Err(e) = conn::write_frame(&mut writer, sess, &msg).await {
                tracing::error!(error = ?e, "pipe 0 write_frame error");
                break;
            }
        }
        closed.store(true, Ordering::SeqCst);
    });
}

async fn read_hello(
    reader: &mut Box<dyn tokio::io::AsyncRead + Unpin + Send>,
) -> BtResult<Message> {
    let deadline = Duration::from_secs(15);
    loop {
        let frame = timeout(deadline, conn::read_frame(reader))
            .await
            .map_err(|_| BtError::ConnectTimeout)?
            .map_err(|_| BtError::ConnectTimeout)?
            .ok_or(BtError::ConnectTimeout)?;
        match frame {
            Incoming::Msg { msg, .. } => return Ok(msg),
            Incoming::Bad { .. } => continue, // 忽略握手期杂帧
        }
    }
}

fn rand_fill(buf: &mut [u8]) {
    use rand::RngExt;
    rand::rng().fill(buf);
}

// ---------------- 调度器 ----------------

/// 心跳：30s 周期 PING，连续 3 次（90s）无响应判定断开（实施方案 4.3-4）。
async fn heartbeat(session: Arc<Session>) {
    let mut iv = tokio::time::interval(Duration::from_secs(30));
    let mut seq = 0u64;
    loop {
        iv.tick().await;
        if session.is_closed() {
            break;
        }
        seq += 1;
        if session.send_control(0, Message::Ping { seq }).is_err() {
            break;
        }
        let silent = now_ms().saturating_sub(session.last_seen_ms.load(Ordering::SeqCst));
        if silent > 90_000 {
            tracing::warn!(conn_id = session.id, "heartbeat timeout, closing");
            session.close(Some(BtError::ConnectTimeout.code()));
            break;
        }
    }
}

/// 中央调度器：按指令码路由所有入站帧，并周期性刷出累积确认。
async fn dispatcher(session: Arc<Session>, mut rx: mpsc::UnboundedReceiver<Disp>) {
    use crate::recv;

    let mut recv_tasks: HashMap<u64, recv::RecvTask> = HashMap::new();
    let mut ack_tick = tokio::time::interval(Duration::from_millis(200));
    ack_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            item = rx.recv() => {
                let Some(disp) = item else { break };
                match disp {
                    Disp::Frame { pipe_id, task_session, frame } => {
                        handle_frame(
                            &session, &mut recv_tasks,
                            pipe_id, task_session, frame,
                        );
                    }
                    Disp::StartRecv { task_session, req } => {
                        if let Err(e) = file::disk::precheck(&session.cfg.save_dir, req.total_size) {
                            let _ = session.send_control(task_session, Message::ErrorMsg {
                                code: e.code(),
                                detail: "磁盘空间不足，已拒收".into(),
                            });
                            session.emit(EngineEvent::Error {
                                conn_id: session.id,
                                task_id: Some(task_session),
                                incoming: true,
                                code: e.code(),
                                message: "磁盘空间不足".into(),
                            });
                            continue;
                        }
                        let total = req.total_size;
                        recv_tasks.insert(task_session, recv::RecvTask::new(req, total, session.cfg.data_dir.clone()));
                        session.emit(EngineEvent::State {
                            conn_id: session.id,
                            task_id: task_session,
                            incoming: true,
                            state: "transferring".into(),
                        });
                    }
                    Disp::FileVerified { task_session, file_seq, result, rel_path } => {
                        recv::on_file_verified(
                            &session, &mut recv_tasks,
                            task_session, file_seq, result, &rel_path,
                        );
                    }
                    Disp::TransportDown { pipe_id } => {
                        if pipe_id == 0 {
                            if !session.is_closed() {
                                tracing::info!(conn_id = session.id, "control pipe down, closing session");
                                session.close(Some(BtError::ConnectTimeout.code()));
                            }
                        } else {
                            // 文件管道读侧终结（对端 FIN/复位）：移除写句柄，
                            // 本侧写任务排空后 FIN，双向终结后流配额归还
                            tracing::debug!(conn_id = session.id, pipe_id, "file pipe down");
                            session.close_file_pipe(pipe_id);
                        }
                    }
                }
            }
            _ = ack_tick.tick() => {
                recv::flush_acks(&session, &mut recv_tasks);
            }
        }
        if session.is_closed() {
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_frame(
    session: &Arc<Session>,
    recv_tasks: &mut HashMap<u64, crate::recv::RecvTask>,
    pipe_id: u64,
    task_session: u64,
    frame: Result<Message, crate::protocol::CodecError>,
) {
    let msg = match frame {
        Err(_) => {
            // 未知指令/畸形包：回 ERROR(-13) 并保持连接（实施方案 4.3-1）
            let _ = session.send_control(
                0,
                Message::ErrorMsg {
                    code: BtError::ProtocolIncompatible.code(),
                    detail: "protocol incompatible or malformed frame".into(),
                },
            );
            return;
        }
        Ok(m) => m,
    };
    session.last_seen_ms.store(now_ms(), Ordering::SeqCst);

    match msg {
        Message::Ping { seq } => {
            let _ = session.send_control(0, Message::Pong { seq });
        }
        Message::Pong { .. } => {}
        Message::Bye => {
            session.close(None);
        }
        Message::Hello { .. } => {} // 会话建立后的迟到帧，忽略
        Message::ErrorMsg { code, detail } => {
            session.emit(EngineEvent::Error {
                conn_id: session.id,
                task_id: if task_session == 0 {
                    None
                } else {
                    Some(task_session)
                },
                // 带任务上下文的 ErrorMsg 帧由接收端发给发送端：
                // 本端是发送方，任务号是本端出站任务的本地号
                incoming: false,
                code,
                message: detail,
            });
        }
        Message::Cancel { reason } => {
            if let Some(f) = session
                .task_cancels
                .lock()
                .unwrap()
                .get(&task_session)
                .cloned()
            {
                f.store(true, Ordering::SeqCst);
            }
            crate::recv::on_cancel(session, recv_tasks, task_session, reason);
            let is_incoming = !session.task_cancels.lock().unwrap().contains_key(&task_session);
            session.emit(EngineEvent::State {
                conn_id: session.id,
                task_id: task_session,
                incoming: is_incoming,
                state: TASK_STATE_CANCELLED.into(),
            });
        }
        Message::PairReq { .. } => {
            handle_late_pair_req(session);
        }
        Message::PairResp { .. } => {
            session.resolve_waiter(&WaitKey::PairResp, msg);
        }
        Message::TransferReq {
            task_uid,
            file_count,
            total_size,
            ..
        } => {
            let info = session.info();
            let req = IncomingTransfer {
                task_uid,
                req_id: task_session,
                conn_id: session.id,
                sender_uuid: info.peer_uuid.clone(),
                sender_name: info.peer_name.clone(),
                file_count,
                total_size,
            };
            let (tx, rxdec) = oneshot::channel();
            session
                .transfer_decisions
                .lock()
                .unwrap()
                .insert(task_session, tx);
            let sess = session.clone();
            let req2 = req.clone();
            tokio::spawn(async move {
                let accept = rxdec.await.unwrap_or(false);
                let _ = sess.send_control(
                    req2.req_id,
                    Message::TransferResp {
                        accept,
                        task_uid: req2.task_uid,
                    },
                );
                if accept {
                    let _ = sess.disp_tx.send(Disp::StartRecv {
                        task_session: req2.req_id,
                        req: req2,
                    });
                }
            });
            session.handler.transfer_incoming(session.clone(), req);
        }
        Message::TransferResp { .. } => {
            session.resolve_waiter(&WaitKey::TransferResp(task_session), msg);
        }
        // 收到文件元数据（FILE_META）：转交接收子系统建档和断点协商
        Message::FileMeta {
            file_seq,
            size,
            mtime,
            rel_path,
            chunk_size,
            ..
        } => {
            crate::recv::on_file_meta(
                session,
                recv_tasks,
                pipe_id,
                task_session,
                file_seq,
                size,
                mtime,
                rel_path,
                chunk_size,
            );
        }
        Message::FileMetaAck { file_seq, .. } => {
            session.resolve_waiter(
                &WaitKey::MetaAck {
                    task_session,
                    file_seq,
                },
                msg,
            );
        }
        Message::Data {
            file_seq,
            chunk_seq,
            payload,
        } => {
            crate::recv::on_data(
                session,
                recv_tasks,
                pipe_id,
                task_session,
                file_seq,
                chunk_seq,
                payload,
            );
        }
        Message::Ack {
            file_seq,
            acked_offset,
        } => {
            tracing::debug!(task_session, file_seq, acked_offset, "received Message::Ack");
            if let Some(tx) = session
                .ack_subs
                .lock()
                .unwrap()
                .get(&(task_session, file_seq))
            {
                let _ = tx.send(acked_offset);
            }
        }
        Message::FileDone { file_seq, hash } => {
            crate::recv::on_file_done(session, recv_tasks, pipe_id, task_session, file_seq, hash);
        }
        Message::FileDoneAck { file_seq, .. } => {
            session.resolve_waiter(
                &WaitKey::DoneAck {
                    task_session,
                    file_seq,
                },
                msg,
            );
        }
    }
}

/// 已建会话上迟到的配对请求（对端认为尚未配对）。
fn handle_late_pair_req(session: &Arc<Session>) {
    let info = session.info();
    let code = pairing::verification_code(&session.identity.fingerprint, &info.peer_fingerprint);
    let (dtx, drx) = oneshot::channel();
    *session.pair_decision.lock().unwrap() = Some(dtx);
    session
        .handler
        .pair_needed(session.clone(), session.id, info, code, false);
    let sess = session.clone();
    tokio::spawn(async move {
        let accept = timeout(Duration::from_secs(300), drx)
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or(false);
        let mut nonce = [0u8; 16];
        rand_fill(&mut nonce);
        let _ = sess.send_control(0, Message::PairResp { accept, nonce });
        if accept {
            let info = sess.info();
            let _ = sess
                .trust
                .add(&info.peer_uuid, &info.peer_fingerprint, &info.peer_name);
        } else {
            sess.close(None);
        }
    });
}
