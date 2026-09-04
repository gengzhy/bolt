//! App 门面：lt-task 对上层（FFI / CLI）的唯一入口（实施方案第九节）。
//!
//! 汇聚配置、身份、信任库、传输引擎、发现、任务队列与事件总线。
//! 所有公共方法线程安全，可从任意线程（含 FFI 回调线程）调用。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use lt_crypto::trust::TrustStore;
use lt_crypto::DeviceIdentity;
use lt_discovery::device_list::DeviceList;
use lt_discovery::manager::{DiscoveryConfig, DiscoveryManager};
use lt_discovery::mdns::AnnounceInfo;
use lt_discovery::nsd_bridge::NsdBridge;
use lt_discovery::Device;
use lt_transfer::engine::{EngineConfig, TransferEngine};
use lt_transfer::protocol::DeviceType;
use lt_transfer::session::{
    EngineEvent, IncomingTransfer, Session, SessionConfig, SessionHandler, SessionInfo,
    TransportKind,
};
use lt_utils::{AppConfig, LtError, LtResult};

use crate::events::LtEvent;
use crate::task::{Direction, TaskRecord, TaskState};

type EventSink = Box<dyn Fn(LtEvent) + Send + Sync>;

/// LocalTransfer 应用门面。
pub struct App {
    rt: tokio::runtime::Runtime,
    cfg: Mutex<AppConfig>,
    identity: Arc<DeviceIdentity>,
    trust: Arc<TrustStore>,
    engine: Mutex<Option<Arc<TransferEngine>>>,
    devices: Arc<DeviceList>,
    discovery: DiscoveryManager,
    nsd: NsdBridge,
    sink: Mutex<Option<EventSink>>,
    tasks: Mutex<HashMap<u64, TaskRecord>>,
    /// uuid → conn_id（活跃会话索引）
    conns: Mutex<HashMap<String, u64>>,
    /// conn_id → 会话
    sessions: Mutex<HashMap<u64, Arc<Session>>>,
    /// 任务取消令牌（发送任务本地令牌）
    cancel_tokens: Mutex<HashMap<u64, Arc<AtomicBool>>>,
    /// 入站传输请求：本地任务号 → (conn_id, 对端线上任务号)
    pending_recv: Mutex<HashMap<u64, (u64, u64)>>,
    /// 入站任务 ID 映射：(conn_id, 对端线上任务号) → 本地任务号。
    /// 两端任务号都从 1 起递增，若直接拿线上号当本地主键，互传时
    /// 入站记录会覆盖同号的本地记录（「传输记录丢失」）；
    /// 键里带上 conn_id，是因为多台设备同时发送时线上任务号会跨设备
    /// 撞车（每台设备的任务号都从 1 递增），只按线上号索引会把两台
    /// 设备的任务事件互相错配。
    recv_ids: Mutex<HashMap<(u64, u64), u64>>,
    next_task_id: AtomicU64,
    /// 发现通道重启在途标志（重启异步执行，避免 FFI 调用线程被 join 阻塞）
    restarting: AtomicBool,
    /// 端口变更触发的引擎重启挂起标志（有任务在途时延迟到任务结束后执行）
    engine_pending_restart: Mutex<bool>,
}

impl App {
    /// 初始化：加载配置/身份/信任库，启动引擎与发现。
    /// `data_dir` 为 None 时使用系统默认数据目录。
    pub fn init(data_dir: Option<PathBuf>) -> LtResult<Arc<App>> {
        lt_crypto::ensure_provider();
        let dir = data_dir.unwrap_or_else(lt_utils::config::default_data_dir);
        let cfg = AppConfig::load(&dir);
        Self::init_with(cfg)
    }

    fn init_with(cfg: AppConfig) -> LtResult<Arc<App>> {
        let cfg_data_dir = cfg.data_dir.clone();
        std::fs::create_dir_all(&cfg.data_dir).ok();
        std::fs::create_dir_all(&cfg.save_dir).ok();
        cfg.save().ok();

        // HELLO 名称取当前配置值（identity 持久化的首启旧名是「随机名称」根因），
        // 不一致时原地改名（uuid/指纹不变）。
        let identity = DeviceIdentity::load_or_create(&cfg.data_dir, &cfg.device_name)?;
        if identity.device_name() != cfg.device_name {
            identity.update_device_name(&cfg.data_dir, &cfg.device_name)?;
        }
        let identity = Arc::new(identity);
        let trust = TrustStore::load(&cfg.data_dir)?;
        let devices = Arc::new(DeviceList::with_own_uuid(identity.uuid.clone()));
        let discovery = DiscoveryManager::new(devices.clone());
        let nsd = NsdBridge::new(devices.clone());

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("lt-rt")
            .build()
            .map_err(|_| LtError::Internal)?;

        let app = Arc::new(App {
            rt,
            cfg: Mutex::new(cfg),
            identity,
            trust: Arc::new(trust),
            engine: Mutex::new(None),
            devices,
            discovery,
            nsd,
            sink: Mutex::new(None),
            tasks: Mutex::new(HashMap::new()),
            conns: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            cancel_tokens: Mutex::new(HashMap::new()),
            pending_recv: Mutex::new(HashMap::new()),
            recv_ids: Mutex::new(HashMap::new()),
            next_task_id: AtomicU64::new(1),
            restarting: AtomicBool::new(false),
            engine_pending_restart: Mutex::new(false),
        });

        // 设备列表变化 → EVT_DEVICE_LIST
        let weak = Arc::downgrade(&app);
        app.devices.set_change_callback(move |devices| {
            if let Some(app) = weak.upgrade() {
                let json = serde_json::to_string(devices).unwrap_or_else(|_| "[]".into());
                app.emit(LtEvent::DeviceList { devices_json: json });
            }
        });

        app.start_engine()?;
        app.restart_discovery();

        // 异步执行 TTL 垃圾回收 (15 天)
        let ttl_data_dir = cfg_data_dir;
        std::thread::spawn(move || {
            let tmp_dir = ttl_data_dir.join("tmp");
            let store = lt_file::resume_store::ResumeStore::new(&ttl_data_dir);
            store.cleanup_stale_cache(&tmp_dir, 15);
        });

        Ok(app)
    }

    fn start_engine(self: &Arc<App>) -> LtResult<()> {
        let cfg = self.cfg.lock().unwrap().clone();
        let engine_cfg = Self::engine_config_from(&cfg);
        std::fs::create_dir_all(&engine_cfg.session.tmp_dir).ok();
        let handler = Arc::new(AppHandler {
            app: Arc::downgrade(self),
        });
        let engine = self.rt.block_on(TransferEngine::start(
            engine_cfg,
            self.identity.clone(),
            self.trust.clone(),
            handler,
        ))?;
        *self.engine.lock().unwrap() = Some(engine);
        Ok(())
    }

    /// 应用配置 → 引擎配置（启动与热更共用，保证语义一致）。
    fn engine_config_from(cfg: &AppConfig) -> EngineConfig {
        let session_cfg = SessionConfig {
            chunk_size: cfg.chunk_size,
            concurrency: cfg.concurrency,
            save_dir: cfg.save_dir.clone(),
            tmp_dir: cfg.data_dir.join("tmp"),
            data_dir: cfg.data_dir.clone(),
            collision: match cfg.collision.as_str() {
                "overwrite" => lt_file::writer::NameCollisionPolicy::Overwrite,
                _ => lt_file::writer::NameCollisionPolicy::AutoRename,
            },
            device_type: device_type(),
        };
        EngineConfig {
            bind_ip: std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            preferred_port: cfg.listen_port,
            session: session_cfg,
            force_tcp: !cfg.prefer_quic,
        }
    }

    /// 端口变更：空闲立即重启引擎；有任务在途则挂起，任务结束后补执行。
    /// 重启会断开既有会话（对端经发现通道按新端口自动重连），在途任务不受影响。
    fn refresh_engine(self: &Arc<App>) {
        if self.engine_busy() {
            *self.engine_pending_restart.lock().unwrap() = true;
            return;
        }
        self.restart_engine_now();
    }

    fn restart_engine_now(self: &Arc<App>) {
        *self.engine_pending_restart.lock().unwrap() = false;
        if let Some(old) = self.engine.lock().unwrap().take() {
            old.shutdown();
        }
        // start_engine 内部用 block_on；事件回调运行在 tokio 工作线程，
        // 不能原地 block_on（会 panic），放到独立线程执行。
        // 重启窗口极短，期间新连接会失败重试；在途任务不受影响。
        let app = self.clone();
        std::thread::spawn(move || {
            if let Err(e) = app.start_engine() {
                tracing::error!(error = %e, "engine restart failed");
            }
        });
    }

    /// 任务到达终态时调用：补执行挂起的引擎重启。
    fn maybe_deferred_engine_restart(self: &Arc<App>) {
        if *self.engine_pending_restart.lock().unwrap() && !self.engine_busy() {
            self.restart_engine_now();
        }
    }

    fn engine_busy(&self) -> bool {
        self.tasks.lock().unwrap().values().any(|t| {
            matches!(
                t.state,
                TaskState::WaitingAccept | TaskState::Transferring | TaskState::Paused
            )
        })
    }

    fn discovery_config(&self) -> DiscoveryConfig {
        let cfg = self.cfg.lock().unwrap().clone();
        let port = self
            .engine
            .lock()
            .unwrap()
            .as_ref()
            .map(|e| e.port)
            .unwrap_or(cfg.listen_port);
        DiscoveryConfig {
            announce: AnnounceInfo {
                uuid: self.identity.uuid.clone(),
                name: cfg.device_name.clone(),
                device_type: device_type() as u8,
                quic_port: port,
                tcp_port: port,
                proto_ver: lt_utils::PROTOCOL_VERSION,
                stealth: cfg.stealth_mode,
                prefer_tcp: !cfg.prefer_quic,
            },
            stealth: cfg.stealth_mode,
            use_mdns: cfg.use_mdns && cfg!(not(target_os = "android")),
            use_udp_probe: true,
            ..Default::default()
        }
    }

    /// 启动/重启发现（配置热更后调用）。
    /// 重启发现通道（配置热更）。
    ///
    /// **异步执行**：stop+start 需要 join 旧线程（最坏数秒），
    /// 若在 UI/FFI 调用线程同步执行会造成卡顿乃至 ANR。
    pub fn restart_discovery(self: &Arc<App>) {
        if self
            .restarting
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return; // 已有一次重启在途
        }
        let me = Arc::clone(self);
        let r = std::thread::Builder::new()
            .name("lt-disc-restart".into())
            .spawn(move || {
                let dcfg = me.discovery_config();
                if let Err(e) = me.discovery.start(dcfg) {
                    me.emit(LtEvent::Error {
                        task_id: None,
                        code: LtError::DiscoveryUnavailable.code(),
                        message: format!("发现通道启动失败：{e}"),
                    });
                }
                me.restarting.store(false, Ordering::SeqCst);
            });
        if r.is_err() {
            self.restarting.store(false, Ordering::SeqCst);
        }
    }

    pub fn stop_discovery(&self) {
        self.discovery.stop();
    }

    /// 立即探测一次网络（即时广播一轮，不重启通道，毫秒级返回）。
    pub fn probe_network(&self) {
        self.discovery.probe_now();
    }

    // ---------------- 事件 ----------------

    pub fn set_event_sink(&self, sink: impl Fn(LtEvent) + Send + Sync + 'static) {
        *self.sink.lock().unwrap() = Some(Box::new(sink));
    }

    fn emit(&self, ev: LtEvent) {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink(ev);
        }
    }

    // ---------------- 设备/连接 ----------------

    pub fn get_devices_json(&self) -> String {
        self.devices.snapshot_json()
    }

    /// Android NsdManager 结果注入桥。
    pub fn nsd(&self) -> &NsdBridge {
        &self.nsd
    }

    /// 手动添加设备（直连场景）。
    pub fn add_manual_device(&self, ip: &str, port: u16) {
        self.discovery.add_manual(ip, port);
    }

    fn engine(&self) -> LtResult<Arc<TransferEngine>> {
        self.engine.lock().unwrap().clone().ok_or(LtError::Internal)
    }

    fn device_addr(&self, uuid: &str) -> LtResult<std::net::SocketAddr> {
        let device = self
            .devices
            .snapshot()
            .into_iter()
            .find(|d| d.uuid == uuid)
            .ok_or(LtError::InvalidArgument)?;
        let ip: std::net::IpAddr = device.ip.parse().map_err(|_| LtError::InvalidArgument)?;
        let port = if device.quic_port != 0 {
            device.quic_port
        } else {
            device.tcp_port
        };
        Ok(std::net::SocketAddr::new(ip, port))
    }

    /// 拨号协议由**发送端（拨号方）自己的设置**决定：本机选 TCP 即走 TCP，
    /// 否则走 QUIC（默认）。接收端双协议并听、来者不拒。协议以发送端
    /// 为准：A→B 用 A 的设置，B→A 用 B 的设置。
    /// `LT_FORCE_TCP` 环境变量在引擎层兜底（联调/排障）。
    fn dial_force_tcp(&self) -> bool {
        !self.cfg.lock().unwrap().prefer_quic
    }

    /// 连接设备（异步发起）。结果经 EVT_CONN_STATE / EVT_ERROR 通知。
    pub fn connect(self: &Arc<App>, uuid: &str) -> LtResult<()> {
        if self.conns.lock().unwrap().contains_key(uuid) {
            return Ok(());
        }
        let target = self.device_addr(uuid)?;
        let force_tcp = self.dial_force_tcp();
        let engine = self.engine()?;
        let weak = Arc::downgrade(self);
        let uuid_owned = uuid.to_string();
        self.rt.spawn(async move {
            match engine.connect_with(target, force_tcp).await {
                // 会话注册与 EVT_CONN_STATE 已由 handler_connected 完成
                Ok(_session) => {}
                Err(e) => {
                    if let Some(app) = weak.upgrade() {
                        app.emit(LtEvent::Error {
                            task_id: None,
                            code: e.code(),
                            message: format!("连接 {uuid_owned} 失败"),
                        });
                    }
                }
            }
        });
        Ok(())
    }

    /// 直连任意地址（不经设备列表）。
    pub fn connect_addr(self: &Arc<App>, ip: &str, port: u16) -> LtResult<()> {
        let addr: std::net::SocketAddr = format!("{ip}:{port}")
            .parse()
            .map_err(|_| LtError::InvalidArgument)?;
        self.add_manual_device(ip, port);
        let force_tcp = self.dial_force_tcp();
        let engine = self.engine()?;
        let weak = Arc::downgrade(self);
        self.rt.spawn(async move {
            match engine.connect_with(addr, force_tcp).await {
                // 会话注册与 EVT_CONN_STATE 已由 handler_connected 完成
                Ok(_session) => {}
                Err(e) => {
                    if let Some(app) = weak.upgrade() {
                        app.emit(LtEvent::Error {
                            task_id: None,
                            code: e.code(),
                            message: format!("连接 {addr} 失败"),
                        });
                    }
                }
            }
        });
        Ok(())
    }

    /// 断开与某设备的会话。
    pub fn disconnect(&self, uuid: &str) -> LtResult<()> {
        let conn_id = self
            .conns
            .lock()
            .unwrap()
            .get(uuid)
            .copied()
            .ok_or(LtError::InvalidArgument)?;
        // 关键：先把 Arc 克隆出来、释放 sessions 锁，再调 close()。
        // close() 会同步回调 handler_disconnected → 再次 lock sessions；
        // 若像 `if let Some(s) = map.lock()...` 那样持锁调用（ scrutinee 临时值
        // 存活到整个 if-let 块结束），同一线程自死锁 → UI 线程 ANR。
        let session = self.sessions.lock().unwrap().get(&conn_id).cloned();
        if let Some(session) = session {
            session.close(None);
        }
        // 引擎侧会话表同步清理（否则已关闭会话滞留其中）
        if let Some(engine) = self.engine.lock().unwrap().as_ref() {
            engine.unregister(conn_id);
        }
        Ok(())
    }

    // ---------------- 配对 / 传输应答 ----------------

    /// 配对应答。pair_id = 会话 conn_id（见 [`App::handler_pair_needed`]）。
    pub fn respond_pair(&self, pair_id: u64, accept: bool) {
        // 同 disconnect：先克隆后释放锁，避免持锁调用会话方法
        let session = self.sessions.lock().unwrap().get(&pair_id).cloned();
        if let Some(session) = session {
            session.respond_pair(pair_id, accept);
        }
    }

    /// 传输请求应答（入站任务；req_id 为本地任务号）。
    pub fn respond_transfer(&self, req_id: u64, accept: bool) {
        // pending_recv 以本地任务号为键，值为 (conn_id, 对端线上任务号)；
        // 应答帧必须携带对端的线上任务号（对端用它索引自己的出站任务）
        let entry = self.pending_recv.lock().unwrap().remove(&req_id);
        if let Some((conn_id, wire_id)) = entry {
            let session = self.sessions.lock().unwrap().get(&conn_id).cloned();
            if let Some(session) = session {
                session.respond_transfer(wire_id, accept);
            }
        }
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(t) = tasks.get_mut(&req_id) {
            if accept {
                t.state = TaskState::Transferring;
            } else {
                t.state = TaskState::Cancelled;
            }
        }
        drop(tasks);
        // 接收任务接受/拒绝后通知上层刷新（此前无事件，UI 看不到状态变化）
        self.emit(LtEvent::TaskState {
            task_id: req_id,
            incoming: true,
            state: if accept { "transferring" } else { "cancelled" }.into(),
        });
    }

    // ---------------- 发送 ----------------

    /// 发送文件给某设备（未连接时自动连接）。返回任务 ID。
    pub fn send_files(self: &Arc<App>, uuid: &str, paths: &[String]) -> LtResult<u64> {
        let items =
            lt_file::traverse::traverse(&paths.iter().map(PathBuf::from).collect::<Vec<_>>())?;
        if items.items.is_empty() {
            return Err(LtError::InvalidArgument);
        }
        let engine = self.engine()?;
        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst);

        let peer_name = self
            .devices
            .snapshot()
            .into_iter()
            .find(|d| d.uuid == uuid)
            .map(|d| d.name)
            .unwrap_or_default();

        let total: u64 = items.items.iter().map(|i| i.size).sum();
        let record = TaskRecord::new_send(
            task_id,
            uuid.to_string(),
            peer_name,
            items.items.len() as u32,
            total,
            paths.to_vec(),
        );
        self.tasks.lock().unwrap().insert(task_id, record);
        // 立即通知上层建档（否则 UI 要等首个状态事件才能看到任务行）
        self.emit(LtEvent::TaskState {
            task_id,
            incoming: false,
            state: "waiting_accept".into(),
        });

        // 已连接：直接发
        if let Some(session) = engine.session_by_uuid(uuid) {
            self.spawn_send(task_id, session, items.items);
            return Ok(task_id);
        }

        // 未连接：先连后发
        let target = self.device_addr(uuid)?;
        let force_tcp = self.dial_force_tcp();
        let weak = Arc::downgrade(self);
        self.rt.spawn(async move {
            let Some(app) = weak.upgrade() else { return };
            match engine.connect_with(target, force_tcp).await {
                Ok(session) => {
                    // 会话注册与 EVT_CONN_STATE 已由 handler_connected 完成
                    app.spawn_send(task_id, session, items.items);
                }
                Err(e) => {
                    let mut tasks = app.tasks.lock().unwrap();
                    if let Some(t) = tasks.get_mut(&task_id) {
                        t.state = TaskState::Error;
                    }
                    drop(tasks);
                    app.emit(LtEvent::Error {
                        task_id: Some(task_id),
                        code: e.code(),
                        message: "发送失败：无法连接对端".into(),
                    });
                }
            }
        });
        Ok(task_id)
    }

    fn spawn_send(
        self: &Arc<App>,
        task_id: u64,
        session: Arc<Session>,
        items: Vec<lt_file::traverse::TransferItem>,
    ) {
        let weak = Arc::downgrade(self);
        let weak_sink = weak.clone();
        let cancel = session.register_task_cancel(task_id);
        self.cancel_tokens
            .lock()
            .unwrap()
            .insert(task_id, cancel.clone());
        // 任务实际使用的传输协议（会话已建立）
        if let Some(t) = self.tasks.lock().unwrap().get_mut(&task_id) {
            t.transport = transport_str(&session.info());
        }
        let sink: lt_transfer::session::EventSink = Arc::new(move |ev| {
            if let Some(app) = weak_sink.upgrade() {
                app.handle_engine_event(ev);
            }
        });
        let peer_uuid = session.info().peer_uuid;
        self.rt.spawn(async move {
            let result = lt_transfer::send::send_files(session, task_id, items, sink, cancel).await;
            let Some(app) = weak.upgrade() else { return };
            let mut tasks = app.tasks.lock().unwrap();
            let Some(t) = tasks.get_mut(&task_id) else {
                return;
            };
            match result {
                Ok(summary) => {
                    if !matches!(t.state, TaskState::Paused | TaskState::Cancelled) {
                        t.state = TaskState::Done;
                    }
                    t.ok_files = summary.ok;
                    t.failed_files = summary.failed;
                }
                Err(e) => {
                    let was_paused = t.state == TaskState::Paused;
                    let was_cancelled = t.state == TaskState::Cancelled;
                    if !was_paused && !was_cancelled {
                        t.state = if e == LtError::Cancelled {
                            TaskState::Cancelled
                        } else {
                            TaskState::Error
                        };
                    }
                    drop(tasks);
                    // 暂停触发的取消不再报错误
                    if !was_paused && !was_cancelled && e != LtError::Cancelled {
                        app.emit(LtEvent::Error {
                            task_id: Some(task_id),
                            code: e.code(),
                            message: format!("发送任务失败（对端 {peer_uuid}）"),
                        });
                    }
                }
            }
        });
    }

    // ---------------- 任务控制 ----------------

    /// 暂停任务：置位取消令牌 → 发送端停止并保留已收部分（可续传）。
    pub fn pause_task(&self, task_id: u64) -> LtResult<()> {
        let incoming;
        let peer_uuid;
        {
            let mut tasks = self.tasks.lock().unwrap();
            let task = tasks.get_mut(&task_id).ok_or(LtError::InvalidArgument)?;
            if matches!(
                task.state,
                TaskState::Done | TaskState::Cancelled | TaskState::Paused
            ) {
                return Err(LtError::InvalidArgument);
            }
            task.state = TaskState::Paused;
            incoming = matches!(task.direction, Direction::Recv);
            peer_uuid = task.peer_uuid.clone();
        }
        if let Some(token) = self.cancel_tokens.lock().unwrap().get(&task_id) {
            token.store(true, Ordering::SeqCst);
        }
        
        let wire_id = if incoming {
            let found = self.recv_ids.lock().unwrap().iter().find(|(_, &local)| local == task_id).map(|(&(_c, wire), _)| wire);
            found.unwrap_or(task_id)
        } else {
            task_id
        };
        let session = self.engine().ok().and_then(|e| e.session_by_uuid(&peer_uuid));
        if let Some(s) = session {
            if incoming {
                s.cancel_task(wire_id, 9);
            }
        }
        
        self.emit(LtEvent::TaskState {
            task_id,
            incoming,
            state: "paused".into(),
        });
        Ok(())
    }

    /// 续传/重传（仅发送端）：重跑 send_files，接收端按已收区间自动跳过。
    /// 暂停后可续传；出错（含对端断开导致的失败）后可重传。
    pub fn resume_task(self: &Arc<App>, task_id: u64) -> LtResult<()> {
        let (peer_uuid, paths) = {
            let mut tasks = self.tasks.lock().unwrap();
            let task = tasks.get_mut(&task_id).ok_or(LtError::InvalidArgument)?;
            if !matches!(task.state, TaskState::Paused | TaskState::Error | TaskState::Cancelled) {
                return Err(LtError::InvalidArgument);
            }
            if task.direction != Direction::Send {
                return Err(LtError::InvalidArgument);
            }
            task.state = TaskState::Transferring;
            task.done_bytes = 0;
            task.ok_files = 0;
            task.failed_files = 0;
            (task.peer_uuid.clone(), task.source_paths.clone())
        };
        let items =
            lt_file::traverse::traverse(&paths.iter().map(PathBuf::from).collect::<Vec<_>>())?;
        if items.items.is_empty() {
            return Err(LtError::FileNotAccessible);
        }
        let engine = self.engine()?;
        let session = engine
            .session_by_uuid(&peer_uuid)
            .ok_or(LtError::ConnectTimeout)?;
        self.spawn_send(task_id, session, items.items);
        Ok(())
    }

    /// 取消任务（不可续传）。
    pub fn cancel_task(&self, task_id: u64) -> LtResult<()> {
        let (incoming, peer_uuid);
        {
            let mut tasks = self.tasks.lock().unwrap();
            let task = tasks.get_mut(&task_id).ok_or(LtError::InvalidArgument)?;
            if matches!(task.state, TaskState::Done | TaskState::Cancelled) {
                return Err(LtError::InvalidArgument);
            }
            task.state = TaskState::Cancelled;
            incoming = matches!(task.direction, Direction::Recv);
            peer_uuid = task.peer_uuid.clone();
        }
        if let Some(token) = self.cancel_tokens.lock().unwrap().get(&task_id) {
            token.store(true, Ordering::SeqCst);
        }
        // 计算对端协议帧使用的任务号：出站任务=本地号；入站任务=线上号（反查映射）
        let wire_id = if incoming {
            let found = self
                .recv_ids
                .lock()
                .unwrap()
                .iter()
                .find(|(_, &local)| local == task_id)
                .map(|(&(_c, wire), _)| wire);
            let Some(wire) = found else {
                // 映射已清理（任务已收尾）：本地置取消态即可，无需通知对端
                self.emit(LtEvent::TaskState {
                    task_id,
                    incoming,
                    state: "cancelled".into(),
                });
                return Ok(());
            };
            wire
        } else {
            task_id
        };
        // 精确通知对端会话（不再向所有会话广播本地号）；
        // 先克隆 Arc 再释放锁，避免持 sessions 锁调用会话方法
        let session = self
            .engine()
            .ok()
            .and_then(|e| e.session_by_uuid(&peer_uuid));
        if let Some(s) = session {
            s.cancel_task(wire_id, 1);
        }
        self.emit(LtEvent::TaskState {
            task_id,
            incoming,
            state: "cancelled".into(),
        });
        Ok(())
    }

    /// 任务列表（按创建时间倒序，同时刻任务号大者在前）。
    pub fn get_tasks_json(&self) -> String {
        let guard = self.tasks.lock().unwrap();
        let mut tasks: Vec<&TaskRecord> = guard.values().collect();
        tasks.sort_by(|a, b| {
            b.created_unix
                .cmp(&a.created_unix)
                .then(b.task_id.cmp(&a.task_id))
        });
        serde_json::to_string(&tasks).unwrap_or_else(|_| "[]".into())
    }

    /// 清除已完成/失败/取消的记录（进行中任务保留）。
    pub fn clear_records(&self) {
        let removed: Vec<u64> = {
            let mut tasks = self.tasks.lock().unwrap();
            let removed: Vec<u64> = tasks
                .iter()
                .filter(|(_, t)| {
                    !matches!(t.state, TaskState::Transferring | TaskState::WaitingAccept)
                })
                .map(|(id, _)| *id)
                .collect();
            for id in &removed {
                tasks.remove(id);
            }
            removed
        };
        // 同步清理入站映射，防止残留映射指向已删除记录
        if !removed.is_empty() {
            self.recv_ids
                .lock()
                .unwrap()
                .retain(|_, local| !removed.contains(local));
        }
    }

    /// 清理临时缓存（临时目录 + 断点记录），实施方案 9.3。
    pub fn clear_temp_cache(&self) -> LtResult<()> {
        let cfg = self.cfg.lock().unwrap().clone();
        let tmp = cfg.data_dir.join("tmp");
        if tmp.exists() {
            std::fs::remove_dir_all(&tmp).map_err(|_| LtError::FileNotAccessible)?;
        }
        std::fs::create_dir_all(&tmp).ok();
        lt_file::resume_store::ResumeStore::new(&cfg.data_dir).clear_all()?;
        Ok(())
    }

    // ---------------- 配置 / 信息 ----------------

    pub fn get_config_json(&self) -> String {
        serde_json::to_string(&*self.cfg.lock().unwrap()).unwrap_or_else(|_| "{}".into())
    }

    /// 合并更新配置并持久化；参数实时生效（不影响在途任务）：
    /// 会话参数（分片/并发/保存目录/冲突策略/传输协议）在此后新建的连接生效，
    /// 发现通道按新配置重启；端口变更空闲立即重启引擎，有任务在途则挂起，
    /// 任务到达终态后补执行（见 maybe_deferred_engine_restart）。
    pub fn set_config(self: &Arc<App>, json: &str) -> LtResult<()> {
        let (merged, old_port) = {
            let base = self.cfg.lock().unwrap().clone();
            (AppConfig::merge_json(base.clone(), json)?, base.listen_port)
        };
        merged.save()?;
        // 改名即时同步到 HELLO（引擎共享同一 identity Arc）
        if merged.device_name != self.identity.device_name() {
            self.identity
                .update_device_name(&merged.data_dir, &merged.device_name)?;
        }
        let port_changed = old_port != merged.listen_port;
        *self.cfg.lock().unwrap() = merged.clone();
        // 引擎配置热更：此后新建的连接/会话按新值生效
        if let Some(engine) = self.engine.lock().unwrap().as_ref() {
            engine.update_config(Self::engine_config_from(&merged));
        }
        self.restart_discovery();
        if port_changed {
            self.refresh_engine();
        }
        Ok(())
    }

    pub fn local_fingerprint(&self) -> String {
        self.identity.fingerprint.clone()
    }

    pub fn device_uuid(&self) -> String {
        self.identity.uuid.clone()
    }

    pub fn trust(&self) -> Arc<TrustStore> {
        self.trust.clone()
    }

    pub fn engine_port(&self) -> u16 {
        self.engine
            .lock()
            .unwrap()
            .as_ref()
            .map(|e| e.port)
            .unwrap_or(0)
    }

    /// 本机设备信息 JSON（uuid/name/dt/qport/tport/ver/stealth/ips）。
    /// Android 侧由 Kotlin 取此信息注册 NSD 服务（Rust 在安卓不跑 mDNS）；
    /// `ips` 为本机全部非环回 IPv4，供设置页展示。
    pub fn local_info_json(&self) -> String {
        let a = self.discovery_config().announce;
        let ips: Vec<String> = lt_utils::net::local_ipv4_nets()
            .iter()
            .map(|(ip, _)| ip.to_string())
            .collect();
        serde_json::json!({
            "uuid": a.uuid,
            "name": a.name,
            "dt": a.device_type,
            "qport": a.quic_port,
            "tport": a.tcp_port,
            "ver": a.proto_ver,
            "stealth": a.stealth,
            "ptcp": a.prefer_tcp,
            "ips": ips,
        })
        .to_string()
    }

    /// 关闭全部（进程退出前调用）。
    pub fn shutdown(&self) {
        self.discovery.stop();
        if let Some(engine) = self.engine.lock().unwrap().take() {
            engine.shutdown();
            // 给 BYE / QUIC CONNECTION_CLOSE 帧一点刷出时间，
            // 让对端走正常断开而不是靠心跳超时（-3）感知
            self.rt.block_on(async {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            });
        }
    }

    // ---------------- 内部：引擎事件 → 任务记录 + 上层事件 ----------------

    /// 引擎事件里的任务号 → 本地任务号。
    /// 入站事件的 task_id 是对端线上号，需经 (conn_id, 线上号) 映射翻译
    /// （多设备同时发送时线上号跨设备撞车，必须用会话区分）；
    /// 出站事件即本地号。
    fn local_task_id(&self, conn_id: u64, wire_id: u64, incoming: bool) -> u64 {
        if incoming {
            self.recv_ids
                .lock()
                .unwrap()
                .get(&(conn_id, wire_id))
                .copied()
                .unwrap_or(wire_id)
        } else {
            wire_id
        }
    }

    fn handle_engine_event(self: &Arc<App>, ev: EngineEvent) {
        match ev {
            EngineEvent::State {
                conn_id,
                task_id,
                incoming,
                state,
            } => {
                let task_id = self.local_task_id(conn_id, task_id, incoming);
                let parsed = TaskState::from_engine(&state);
                let mut tasks = self.tasks.lock().unwrap();
                if let Some(t) = tasks.get_mut(&task_id) {
                    if !matches!(t.state, TaskState::Paused | TaskState::Cancelled) {
                        t.state = parsed;
                    }
                } else if incoming {
                    // 接收任务兜底建档（正常路径在 transfer_incoming 已建）
                    tasks.insert(
                        task_id,
                        TaskRecord::new_recv(task_id, String::new(), String::new(), 0, 0),
                    );
                }
                drop(tasks);
                self.emit(LtEvent::TaskState {
                    task_id,
                    incoming,
                    state,
                });
                // 端口变更挂起的引擎重启：任务全部到达终态后补执行
                self.maybe_deferred_engine_restart();
            }
            EngineEvent::Progress {
                conn_id,
                task_id,
                incoming,
                rel_path,
                done,
                total,
                rate_bps,
                eta_secs,
            } => {
                let task_id = self.local_task_id(conn_id, task_id, incoming);
                {
                    let mut tasks = self.tasks.lock().unwrap();
                    if let Some(t) = tasks.get_mut(&task_id) {
                        t.done_bytes = done;
                        t.rate_bps = rate_bps;
                        t.eta_secs = eta_secs;
                        if let Some(rel) = &rel_path {
                            t.current_file = rel.clone();
                        }
                        if t.state == TaskState::WaitingAccept {
                            t.state = TaskState::Transferring;
                        }
                    }
                }
                self.emit(LtEvent::TaskProgress {
                    task_id,
                    incoming,
                    rel_path,
                    done,
                    total,
                    rate_bps,
                    eta_secs,
                });
            }
            EngineEvent::FileFinished {
                conn_id,
                task_id,
                incoming,
                rel_path: _,
                ok,
            } => {
                let task_id = self.local_task_id(conn_id, task_id, incoming);
                let mut tasks = self.tasks.lock().unwrap();
                if let Some(t) = tasks.get_mut(&task_id) {
                    if ok {
                        t.ok_files += 1;
                    } else {
                        t.failed_files += 1;
                    }
                }
            }
            EngineEvent::Summary {
                conn_id,
                task_id,
                incoming,
                ok,
                failed,
            } => {
                let task_id = self.local_task_id(conn_id, task_id, incoming);
                {
                    let mut tasks = self.tasks.lock().unwrap();
                    if let Some(t) = tasks.get_mut(&task_id) {
                        t.ok_files = ok;
                        t.failed_files = failed;
                        if !matches!(t.state, TaskState::Paused | TaskState::Cancelled) {
                            t.state = TaskState::Done;
                            t.done_bytes = t.total_size;
                        }
                    }
                }
                self.cancel_tokens.lock().unwrap().remove(&task_id);
                self.pending_recv.lock().unwrap().remove(&task_id);
                self.emit(LtEvent::TaskSummary {
                    task_id,
                    incoming,
                    ok,
                    failed,
                });
                // 端口变更挂起的引擎重启：任务全部到达终态后补执行
                self.maybe_deferred_engine_restart();
            }
            EngineEvent::Error {
                conn_id,
                task_id,
                incoming,
                code,
                message,
            } => {
                let task_id = task_id.map(|id| self.local_task_id(conn_id, id, incoming));
                self.emit(LtEvent::Error {
                    task_id,
                    code,
                    message,
                });
            }
        }
    }

    // ---------------- 内部：SessionHandler 入口 ----------------

    fn handler_connected(self: &Arc<App>, session: Arc<Session>, info: SessionInfo) {
        self.conns
            .lock()
            .unwrap()
            .insert(info.peer_uuid.clone(), info.conn_id);
        self.sessions.lock().unwrap().insert(info.conn_id, session);

        // HELLO 校正：以对端真实 uuid 写入设备列表（手动连接的 manual:<ip>
        // 占位条目替换掉），并钉住——连接期间条目不得被 TTL 清扫移除。
        // 注意：info.addr 对「被连方」是对端的临时源端口，不能拿它覆盖
        // 设备条目里的真实监听端口（会指向对端临时端口导致后续拨号失败），
        // 已有条目一律保留端口与协议偏好；新条目才用 addr 端口兜底。
        let (ip, port) = match info.addr.rsplit_once(':') {
            Some((h, p)) => (
                h.trim_start_matches('[').trim_end_matches(']').to_string(),
                p.parse().unwrap_or(lt_utils::DEFAULT_PORT),
            ),
            None => (info.addr.clone(), lt_utils::DEFAULT_PORT),
        };
        let existing = self
            .devices
            .snapshot()
            .into_iter()
            .find(|d| d.uuid == info.peer_uuid);
        let (quic_port, tcp_port, prefer_tcp) = match &existing {
            Some(d) => (d.quic_port, d.tcp_port, d.prefer_tcp),
            None => (port, port, false),
        };
        self.devices.upsert(Device {
            uuid: info.peer_uuid.clone(),
            name: info.peer_name.clone(),
            device_type: info.peer_type as u8,
            ip: ip.clone(),
            quic_port,
            tcp_port,
            proto_ver: lt_utils::PROTOCOL_VERSION,
            stealth: false,
            prefer_tcp,
            source: "conn".into(),
            last_seen_unix: Device::now_unix(),
        });
        self.devices.remove(&format!("manual:{ip}"));
        self.devices.pin(&info.peer_uuid);

        self.emit(LtEvent::ConnState {
            uuid: info.peer_uuid.clone(),
            name: info.peer_name.clone(),
            state: "connected".into(),
            conn_id: info.conn_id,
            transport: transport_str(&info),
            err: None,
        });

        // 断线重连自动恢复：扫描属于该对端且因断网等异常处于 Error 态的出站任务并自动续传
        let to_resume: Vec<u64> = self
            .tasks
            .lock()
            .unwrap()
            .values()
            .filter(|t| {
                t.peer_uuid == info.peer_uuid
                    && t.direction == Direction::Send
                    && t.state == TaskState::Error
            })
            .map(|t| t.task_id)
            .collect();
        for task_id in to_resume {
            tracing::info!(task_id, peer = %info.peer_uuid, "auto-resuming interrupted task on reconnect");
            let app = self.clone();
            self.rt.spawn(async move {
                let _ = app.resume_task(task_id);
            });
        }
    }

    fn handler_pair_needed(
        &self,
        session: Arc<Session>,
        pair_id: u64,
        info: SessionInfo,
        code: String,
    ) {
        self.sessions.lock().unwrap().insert(info.conn_id, session);
        self.emit(LtEvent::PairRequest {
            pair_id,
            uuid: info.peer_uuid,
            name: info.peer_name,
            code,
        });
    }

    fn handler_transfer_incoming(&self, session: Arc<Session>, req: IncomingTransfer) {
        // 「已信任设备自动接收」开关打开且发送端是已配对设备 → 免确认自动接收。
        // 注意不能用 is_auto_receive（那是逐设备开关，配对时恒为 false、
        // 也无 UI 可设，用它做条件会导致开关永远不生效——实测 bug）。
        let auto =
            self.cfg.lock().unwrap().auto_accept_trusted && self.trust.is_paired(&req.sender_uuid);
        
        // 1. 查找是否为历史任务的续传（断网重连复用）
        let mut local_id = None;
        {
            let mut tasks = self.tasks.lock().unwrap();
            // 倒序查找，优先复用最近的匹配任务
            let mut keys: Vec<u64> = tasks.keys().cloned().collect();
            keys.sort_unstable_by(|a, b| b.cmp(a));
            for id in keys {
                let t = tasks.get_mut(&id).unwrap();
                if t.direction == Direction::Recv
                    && t.peer_uuid == req.sender_uuid
                    && t.file_count == req.file_count
                    && t.total_size == req.total_size
                {
                    if t.state == TaskState::Transferring {
                        // 并发重传竞态拦截 (Concurrent Race Condition Blocking)
                        // 已有一个活着的相同特征任务，拒绝本次重复的握手请求
                        session.respond_transfer(req.req_id, false);
                        return;
                    }
                    if matches!(t.state, TaskState::Paused | TaskState::Error) {
                        local_id = Some(id);
                        // 立即抢占状态，防止后续并发进入
                        t.state = if auto { TaskState::Transferring } else { TaskState::WaitingAccept }; 
                        t.transport = transport_str(&session.info());
                        break;
                    }
                }
            }
        }

        let local_id = if let Some(id) = local_id {
            // 清理可能的悬空旧映射（旧的 conn_id）并更新新映射
            self.recv_ids.lock().unwrap().retain(|_, v| *v != id);
            self.recv_ids.lock().unwrap().insert((req.conn_id, req.req_id), id);
            id
        } else {
            // 入站任务分配本地任务号，并以「(conn_id, 线上号) → 本地号」登记映射。
            let id = self.next_task_id.fetch_add(1, Ordering::SeqCst);
            self.recv_ids
                .lock()
                .unwrap()
                .insert((req.conn_id, req.req_id), id);
            let mut record = TaskRecord::new_recv(
                id,
                req.sender_uuid.clone(),
                req.sender_name.clone(),
                req.file_count,
                req.total_size,
            );
            record.transport = transport_str(&session.info());
            self.tasks.lock().unwrap().insert(id, record);
            id
        };
        if auto {
            // 应答帧带线上号（对端索引用的是它自己的任务号）
            session.respond_transfer(req.req_id, true);
            if let Some(t) = self.tasks.lock().unwrap().get_mut(&local_id) {
                t.state = TaskState::Transferring;
            }
            return;
        }
        self.pending_recv
            .lock()
            .unwrap()
            .insert(local_id, (req.conn_id, req.req_id));
        self.sessions.lock().unwrap().insert(req.conn_id, session);
        self.emit(LtEvent::TransferRequest {
            req_id: local_id,
            uuid: req.sender_uuid,
            name: req.sender_name,
            file_count: req.file_count,
            total_size: req.total_size,
        });
    }

    fn handler_disconnected(&self, conn_id: u64, err_code: Option<i32>) {
        let uuid = {
            let mut conns = self.conns.lock().unwrap();
            let uuid = conns
                .iter()
                .find(|(_, &c)| c == conn_id)
                .map(|(u, _)| u.clone());
            if let Some(u) = &uuid {
                conns.remove(u);
            }
            uuid
        };
        self.sessions.lock().unwrap().remove(&conn_id);
        if let Some(u) = &uuid {
            // 解除钉住：断开后该设备恢复 TTL 清扫
            self.devices.unpin(u);

            // 僵尸状态自动剔除 (Zombie State Auto-recovery)
            // 找出属于该断开设备的所有处于传输中/等待中的任务，强制置为 Error
            let mut tasks = self.tasks.lock().unwrap();
            for (id, t) in tasks.iter_mut() {
                if t.peer_uuid == *u && matches!(t.state, TaskState::Transferring | TaskState::WaitingAccept) {
                    t.state = TaskState::Error;
                    self.emit(LtEvent::TaskState {
                        task_id: *id,
                        incoming: t.direction == Direction::Recv,
                        state: "error".into(),
                    });
                }
            }
        }
        let name = self
            .devices
            .snapshot()
            .into_iter()
            .find(|d| Some(&d.uuid) == uuid.as_ref())
            .map(|d| d.name)
            .unwrap_or_default();
        self.emit(LtEvent::ConnState {
            uuid: uuid.unwrap_or_default(),
            name,
            state: "disconnected".into(),
            conn_id,
            transport: String::new(),
            err: err_code,
        });
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.discovery.stop();
    }
}

/// SessionHandler 实现（弱引用回 App，避免循环引用）。
struct AppHandler {
    app: Weak<App>,
}

impl SessionHandler for AppHandler {
    fn connected(&self, session: Arc<Session>, info: SessionInfo) {
        if let Some(app) = self.app.upgrade() {
            app.handler_connected(session, info);
        }
    }
    fn pair_needed(&self, session: Arc<Session>, pair_id: u64, info: SessionInfo, code: String) {
        if let Some(app) = self.app.upgrade() {
            app.handler_pair_needed(session, pair_id, info, code);
        }
    }
    fn transfer_incoming(&self, session: Arc<Session>, req: IncomingTransfer) {
        if let Some(app) = self.app.upgrade() {
            app.handler_transfer_incoming(session, req);
        }
    }
    fn disconnected(&self, conn_id: u64, err_code: Option<i32>) {
        if let Some(app) = self.app.upgrade() {
            app.handler_disconnected(conn_id, err_code);
        }
    }
    fn event(&self, ev: EngineEvent) {
        if let Some(app) = self.app.upgrade() {
            app.handle_engine_event(ev);
        }
    }
}

fn transport_str(info: &SessionInfo) -> String {
    match info.transport {
        TransportKind::Quic => "quic".into(),
        TransportKind::Tcp => "tcp".into(),
    }
}

fn device_type() -> DeviceType {
    lt_transfer::engine::device_type()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_init_loopback() {
        // 完整启动：身份生成、引擎监听、发现启动；验证指纹与端口可用
        let dir = std::env::temp_dir().join(format!("lt_task_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let app = App::init(Some(dir.clone())).expect("init");
        assert!(!app.local_fingerprint().is_empty());
        assert!(app.engine_port() >= lt_utils::DEFAULT_PORT);
        assert!(!app.device_uuid().is_empty());
        // 配置读写
        app.set_config(r#"{"device_name":"测试机"}"#).unwrap();
        assert!(app.get_config_json().contains("测试机"));
        // 空任务列表
        assert_eq!(app.get_tasks_json(), "[]");
        app.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 传输协议二选一：拨号协议由**发送端（本机）设置**决定，默认 QUIC。
    /// A→B 用 A 的设置，B→A 用 B 的设置；接收端双协议并听、来者不拒。
    #[test]
    fn protocol_sender_driven() {
        let dir = std::env::temp_dir().join(format!("lt_proto_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let app = App::init(Some(dir.clone())).expect("init");

        // 默认 QUIC
        assert!(!app.dial_force_tcp());

        // 本机切 TCP → 本机作为发送端即走 TCP
        app.set_config(r#"{"prefer_quic":false}"#).unwrap();
        assert!(app.dial_force_tcp());

        // 切回 QUIC → 恢复 QUIC
        app.set_config(r#"{"prefer_quic":true}"#).unwrap();
        assert!(!app.dial_force_tcp());

        app.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
