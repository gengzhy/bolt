//! 传输引擎门面：监听（QUIC + TCP）与拨号（QUIC 优先，失败降级 TCP）。
//!
//! 端口策略：默认 8899，占用时自动递增尝试 8900~8950（实施方案 6.4）。

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crypto::trust::TrustStore;
use crypto::DeviceIdentity;
use utils::{BtError, BtResult};

use crate::protocol::DeviceType;
use crate::session::{HandshakeCtx, Session, SessionConfig, SessionHandler};
use crate::{quic, session, tcp};

/// 引擎配置。
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub bind_ip: IpAddr,
    pub preferred_port: u16,
    pub session: SessionConfig,
    /// 拨号协议（对应设置「传输协议」二选一）：true=TCP / false=QUIC（默认）；
    /// 选定协议失败直接报错，不自动降级
    pub force_tcp: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            bind_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            preferred_port: utils::DEFAULT_PORT,
            session: SessionConfig::default(),
            force_tcp: false,
        }
    }
}

/// Bolt 传输引擎（每进程一个）。
pub struct TransferEngine {
    identity: Arc<DeviceIdentity>,
    trust: Arc<TrustStore>,
    handler: Arc<dyn SessionHandler>,
    /// Mutex 包裹：配置热更（set_config 后新连接/新会话立即按新值生效，
    /// 已建立的会话与在途任务不受影响）。
    cfg: Mutex<EngineConfig>,
    /// 实际监听端口（可能因占用而递增）
    pub port: u16,
    sessions: Mutex<HashMap<u64, Arc<Session>>>,
    /// QUIC/TCP 监听任务句柄：shutdown 时中止，释放端口
    /// （否则引擎重启后旧监听仍占用旧端口）
    listeners: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    next_conn_id: AtomicU64,
    stopped: AtomicBool,
}

impl TransferEngine {
    /// 启动引擎：绑定端口（自动避让），开启 QUIC 与 TCP 双监听。
    pub async fn start(
        cfg: EngineConfig,
        identity: Arc<DeviceIdentity>,
        trust: Arc<TrustStore>,
        handler: Arc<dyn SessionHandler>,
    ) -> BtResult<Arc<TransferEngine>> {
        crypto::ensure_provider();
        let port = utils::net::find_available_port(cfg.preferred_port, cfg.bind_ip)?;
        let bind_ip = cfg.bind_ip;
        let engine = Arc::new(TransferEngine {
            identity,
            trust,
            handler,
            cfg: Mutex::new(cfg),
            port,
            sessions: Mutex::new(HashMap::new()),
            listeners: Mutex::new(Vec::new()),
            next_conn_id: AtomicU64::new(1),
            stopped: AtomicBool::new(false),
        });

        let addr = SocketAddr::new(bind_ip, port);

        // QUIC 监听
        let ep = quic::server_endpoint(addr, &engine.identity)?;
        let e2 = engine.clone();
        let h2 = tokio::spawn(async move {
            loop {
                let Some(incoming) = ep.accept().await else {
                    break;
                };
                let Ok(connecting) = incoming.accept() else {
                    continue;
                };
                let e2 = e2.clone();
                tokio::spawn(async move {
                    let Ok(connection) = connecting.await else {
                        return;
                    };
                    let peer = connection.remote_address();
                    // 首条双向流 = 控制流
                    let Ok((send, recv)) = connection.accept_bi().await else {
                        return;
                    };
                    let pipe = quic::pipe_from_bi(send, recv);
                    let conn_id = e2.next_conn_id.fetch_add(1, Ordering::SeqCst);
                    let ctx = HandshakeCtx {
                        identity: e2.identity.clone(),
                        trust: e2.trust.clone(),
                        handler: e2.handler.clone(),
                        cfg: e2.cfg.lock().unwrap().session.clone(),
                        kind: crate::session::TransportKind::Quic,
                        conn_id,
                        addr: peer.to_string(),
                        quic: Some(connection.clone()),
                        quic_ep: None, // 服务端 Endpoint 由监听循环持有
                        tcp_kill: None,
                    };
                    match session::handshake_in(ctx, pipe).await {
                        // 后续入站文件流由 finish_handshake 内的 accept 循环
                        // 对称处理（拨号端同样需要，见 session.rs）
                        Ok(sess) => e2.register(sess),
                        Err(e) => {
                            tracing::debug!(error = %e, %peer, "quic inbound handshake failed");
                        }
                    }
                });
            }
        });

        // TCP 监听
        let listener = tcp::listen(addr).await?;
        let acceptor = tcp::acceptor(&engine.identity)?;
        let e3 = engine.clone();
        let h3 = tokio::spawn(async move {
            loop {
                let Ok((pipe, peer, kill)) = tcp::accept_one(&listener, &acceptor).await else {
                    continue;
                };
                if e3.stopped.load(Ordering::SeqCst) {
                    break;
                }
                let e3 = e3.clone();
                tokio::spawn(async move {
                    let conn_id = e3.next_conn_id.fetch_add(1, Ordering::SeqCst);
                    let ctx = HandshakeCtx {
                        identity: e3.identity.clone(),
                        trust: e3.trust.clone(),
                        handler: e3.handler.clone(),
                        cfg: e3.cfg.lock().unwrap().session.clone(),
                        kind: crate::session::TransportKind::Tcp,
                        conn_id,
                        addr: peer.to_string(),
                        quic: None,
                        quic_ep: None,
                        tcp_kill: Some(Arc::new(kill)),
                    };
                    match session::handshake_in(ctx, pipe).await {
                        Ok(sess) => e3.register(sess),
                        Err(e) => {
                            tracing::debug!(error = %e, %peer, "tcp inbound handshake failed");
                        }
                    }
                });
            }
        });

        engine.listeners.lock().unwrap().push(h2);
        engine.listeners.lock().unwrap().push(h3);

        tracing::info!(port, "transfer engine started");
        Ok(engine)
    }

    fn register(&self, session: Arc<Session>) {
        let mut sessions = self.sessions.lock().unwrap();
        // 顺带清扫已关闭会话（对端主动断开等路径不会显式注销，避免滞留）
        sessions.retain(|_, s| !s.is_closed());
        sessions.insert(session.id, session);
    }

    /// 注销会话（主动断开时调用）。
    pub fn unregister(&self, conn_id: u64) {
        self.sessions.lock().unwrap().remove(&conn_id);
    }

    pub fn get_session(&self, conn_id: u64) -> Option<Arc<Session>> {
        self.sessions.lock().unwrap().get(&conn_id).cloned()
    }

    /// 按 UUID 查找会话。
    pub fn session_by_uuid(&self, uuid: &str) -> Option<Arc<Session>> {
        self.sessions
            .lock()
            .unwrap()
            .values()
            .find(|s| !s.is_closed() && s.info().peer_uuid == uuid)
            .cloned()
    }

    /// 拨号：按本机设置「传输协议」二选一（QUIC 默认 / TCP），选定协议失败
    /// 直接报错，不做自动降级。环境变量 `BT_FORCE_TCP=1` 强制 TCP（联调/排障用）。
    pub async fn connect(&self, target: SocketAddr) -> BtResult<Arc<Session>> {
        // 关键：先把 force_tcp 读出本地、释放 cfg 锁，再调 connect_with。
        // 若写成 `self.connect_with(target, self.cfg.lock()..force_tcp).await`，
        // MutexGuard 临时值会存活到整个 .await 结束，而 connect_with 内部
        // 构造 HandshakeCtx 时再次 lock 同一把 std::sync::Mutex → 同线程自死锁。
        let force_tcp = self.cfg.lock().unwrap().force_tcp;
        self.connect_with(target, force_tcp).await
    }

    /// 拨号（显式指定协议）：`force_tcp` 为最终选择——上层取「本机设置 ∨
    /// 对端广播的协议偏好」的并集，任一端选了 TCP 即用 TCP，保证对端
    /// 设置页的协议选择同样生效（监听侧双协议并听，谁拨号谁说了算会让
    /// 另一端的选择被静默忽略）。
    pub async fn connect_with(
        &self,
        target: SocketAddr,
        force_tcp: bool,
    ) -> BtResult<Arc<Session>> {
        // 已连接则复用
        // （由调用方按 uuid 判断，此处不感知）

        let force_tcp = force_tcp
            || std::env::var("BT_FORCE_TCP")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

        if !force_tcp {
            // QUIC 唯一通道（4s 超时）：失败即抛错，不降级 TCP
            let (pipe, connection, endpoint) = match tokio::time::timeout(
                Duration::from_secs(4),
                quic::dial(target, &self.identity),
            )
            .await
            {
                Ok(Ok(res)) => res,
                Ok(Err(e)) => {
                    tracing::debug!(error = %e, "quic dial failed");
                    return Err(BtError::ConnectTimeout);
                }
                Err(_) => {
                    tracing::debug!("quic dial timeout");
                    return Err(BtError::ConnectTimeout);
                }
            };
            let conn_id = self.next_conn_id.fetch_add(1, Ordering::SeqCst);
            let ctx = HandshakeCtx {
                identity: self.identity.clone(),
                trust: self.trust.clone(),
                handler: self.handler.clone(),
                cfg: self.cfg.lock().unwrap().session.clone(),
                kind: crate::session::TransportKind::Quic,
                conn_id,
                addr: target.to_string(),
                quic: Some(connection),
                quic_ep: Some(endpoint), // Endpoint 须随会话存活，否则驱动终止
                tcp_kill: None,
            };
            let sess = session::handshake_out(ctx, pipe).await?;
            self.register(sess.clone());
            return Ok(sess);
        }

        // TCP 唯一通道（6s 超时）
        let (pipe, kill) =
            tokio::time::timeout(Duration::from_secs(6), tcp::dial(target, &self.identity))
                .await
                .map_err(|_| BtError::ConnectTimeout)?
                .map_err(|_| BtError::ConnectTimeout)?;
        let conn_id = self.next_conn_id.fetch_add(1, Ordering::SeqCst);
        let ctx = HandshakeCtx {
            identity: self.identity.clone(),
            trust: self.trust.clone(),
            handler: self.handler.clone(),
            cfg: self.cfg.lock().unwrap().session.clone(),
            kind: crate::session::TransportKind::Tcp,
            conn_id,
            addr: target.to_string(),
            quic: None,
            quic_ep: None,
            tcp_kill: Some(Arc::new(kill)),
        };
        let sess = session::handshake_out(ctx, pipe).await?;
        self.register(sess.clone());
        Ok(sess)
    }

    /// 配置热更：仅影响此后新建的连接/会话，在途任务不受影响。
    pub fn update_config(&self, cfg: EngineConfig) {
        *self.cfg.lock().unwrap() = cfg;
    }

    /// 停止引擎：中止监听任务（释放端口）并关闭全部会话。
    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        for h in self.listeners.lock().unwrap().drain(..) {
            h.abort();
        }
        let sessions: Vec<Arc<Session>> = self.sessions.lock().unwrap().values().cloned().collect();
        for s in sessions {
            s.close(None);
        }
    }
}

/// 会话配置中的设备类型（由上层注入）。
pub fn device_type() -> DeviceType {
    if cfg!(windows) {
        DeviceType::Windows
    } else if cfg!(target_os = "linux") {
        DeviceType::Linux
    } else {
        DeviceType::Android
    }
}
