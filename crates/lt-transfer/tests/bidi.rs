//! 回归测试：同一会话上的双向传输（实施方案 4.3 会话复用）。
//!
//! 修复前缺陷：QUIC 拨号端没有对入站文件流的 accept 循环，
//! 被连端沿原会话反向发送时，其打开的流无人接收 → FileMetaAck
//! 永不返回 → 30s 超时（-3「连接超时或远端设备离线」），且永远失败。
//! 本测试固定覆盖「拨号端先发 → 被连端沿同一会话反向发」路径。

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lt_crypto::trust::TrustStore;
use lt_crypto::DeviceIdentity;
use lt_transfer::engine::{EngineConfig, TransferEngine};
use lt_transfer::session::{
    EngineEvent, EventSink, IncomingTransfer, Session, SessionConfig, SessionHandler, SessionInfo,
};

/// 全自动应答处理器：自动接受配对与传输请求，并记录会话/接收汇总。
#[derive(Default)]
struct AutoHandler {
    session: Mutex<Option<Arc<Session>>>,
    summary: Mutex<Option<(u32, u32)>>,
    /// 累计收到的 Summary 次数（并发任务回归用）
    summaries: Mutex<usize>,
}

impl SessionHandler for AutoHandler {
    fn connected(&self, session: Arc<Session>, _info: SessionInfo) {
        *self.session.lock().unwrap() = Some(session);
    }
    fn pair_needed(&self, session: Arc<Session>, pair_id: u64, _info: SessionInfo, _code: String) {
        session.respond_pair(pair_id, true);
    }
    fn transfer_incoming(&self, session: Arc<Session>, req: IncomingTransfer) {
        session.respond_transfer(req.req_id, true);
    }
    fn event(&self, ev: EngineEvent) {
        if let EngineEvent::Summary {
            incoming: true,
            ok,
            failed,
            ..
        } = ev
        {
            *self.summary.lock().unwrap() = Some((ok, failed));
            *self.summaries.lock().unwrap() += 1;
        }
    }
}

fn engine_cfg(dir: &std::path::Path, port: u16) -> EngineConfig {
    EngineConfig {
        bind_ip: "127.0.0.1".parse().unwrap(),
        preferred_port: port,
        session: SessionConfig {
            save_dir: dir.join("received"),
            tmp_dir: dir.join("tmp"),
            data_dir: dir.to_path_buf(),
            ..Default::default()
        },
        force_tcp: false,
    }
}

fn make_item(path: &std::path::Path, name: &str) -> lt_file::traverse::TransferItem {
    let meta = std::fs::metadata(path).unwrap();
    lt_file::traverse::TransferItem {
        abs_path: path.to_path_buf(),
        rel_path: name.to_string(),
        size: meta.len(),
        mtime_unix: 0,
        head_hash: [0; 16],
    }
}

async fn wait_summary(h: &AutoHandler) -> (u32, u32) {
    for _ in 0..600 {
        if let Some(s) = *h.summary.lock().unwrap() {
            return s;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("等待接收录取汇总超时（60s）");
}

async fn wait_session(h: &AutoHandler) -> Arc<Session> {
    for _ in 0..200 {
        if let Some(s) = h.session.lock().unwrap().clone() {
            return s;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("等待会话建立超时（10s）");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bidi_transfer_on_same_session() {
    let root = std::env::temp_dir().join(format!("lt_bidi_{}", std::process::id()));
    let dir_a = root.join("a");
    let dir_b = root.join("b");
    let src = root.join("src");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();
    std::fs::create_dir_all(&src).unwrap();
    let f1 = src.join("f1.bin");
    let f2 = src.join("f2.bin");
    std::fs::write(&f1, vec![7u8; 300_000]).unwrap();
    std::fs::write(&f2, vec![9u8; 200_000]).unwrap();

    let id_a = Arc::new(DeviceIdentity::load_or_create(&dir_a, "node-a").unwrap());
    let id_b = Arc::new(DeviceIdentity::load_or_create(&dir_b, "node-b").unwrap());
    let trust_a = Arc::new(TrustStore::load(&dir_a).unwrap());
    let trust_b = Arc::new(TrustStore::load(&dir_b).unwrap());

    let h_a = Arc::new(AutoHandler::default());
    let h_b = Arc::new(AutoHandler::default());

    let engine_a = TransferEngine::start(
        engine_cfg(&dir_a, 8961),
        id_a,
        trust_a,
        h_a.clone() as Arc<dyn SessionHandler>,
    )
    .await
    .unwrap();
    let engine_b = TransferEngine::start(
        engine_cfg(&dir_b, 8962),
        id_b,
        trust_b,
        h_b.clone() as Arc<dyn SessionHandler>,
    )
    .await
    .unwrap();

    let addr_a: std::net::SocketAddr = format!("127.0.0.1:{}", engine_a.port).parse().unwrap();

    // B 拨号 → A（B 为拨号端，A 为被连端）
    let sess_b = engine_b.connect(addr_a).await.unwrap();
    assert_eq!(sess_b.transport(), lt_transfer::TransportKind::Quic);
    let sess_a = wait_session(&h_a).await;

    let noop_sink: EventSink = Arc::new(|_| {});
    let cancel = Arc::new(AtomicBool::new(false));

    // 1) 拨号端 B → A（修复前即可成功）
    let sum = lt_transfer::send::send_files(
        sess_b.clone(),
        1,
        vec![make_item(&f1, "f1.bin")],
        noop_sink.clone(),
        cancel.clone(),
    )
    .await
    .unwrap();
    assert_eq!((sum.ok, sum.failed), (1, 0));
    assert_eq!(wait_summary(&h_a).await, (1, 0));
    assert_eq!(
        std::fs::read(dir_a.join("received/f1.bin")).unwrap().len(),
        300_000
    );

    // 2) 反向：被连端 A → B，复用同一会话（修复前 30s 超时 -3）
    let sum = lt_transfer::send::send_files(
        sess_a.clone(),
        2,
        vec![make_item(&f2, "f2.bin")],
        noop_sink.clone(),
        cancel.clone(),
    )
    .await
    .unwrap();
    assert_eq!((sum.ok, sum.failed), (1, 0));
    assert_eq!(wait_summary(&h_b).await, (1, 0));
    assert_eq!(
        std::fs::read(dir_b.join("received/f2.bin")).unwrap().len(),
        200_000
    );

    // 3) 再正向一次：确认文件管道 FIN/清理没有损伤会话
    let sum = lt_transfer::send::send_files(
        sess_b.clone(),
        3,
        vec![make_item(&f1, "f1.bin")],
        noop_sink,
        cancel,
    )
    .await
    .unwrap();
    assert_eq!((sum.ok, sum.failed), (1, 0));

    engine_a.shutdown();
    engine_b.shutdown();
    let _ = std::fs::remove_dir_all(&root);
}

/// 回归测试：同一会话上「并发」两个发送任务（bug 2「多人无传输 / -1 参数无效」）。
///
/// 修复前缺陷：接收端用全局 `file_seq → 任务号` 单键索引，发送端用
/// `file_seq` 单键等待 FileMetaAck/FileDoneAck 与订阅 ACK。两个任务并发时
/// file_seq 都从 0 递增，互相覆盖 → 数据路由到错误文件 → 写入偏移越界
/// 抛 -1「写入失败」。修复后按 (任务号, 文件序号) 两级索引。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_two_tasks_on_same_session() {
    let root = std::env::temp_dir().join(format!("lt_conc_{}", std::process::id()));
    let dir_a = root.join("a");
    let dir_b = root.join("b");
    let src = root.join("src");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();
    std::fs::create_dir_all(&src).unwrap();
    let f1 = src.join("f1.bin");
    let f2 = src.join("f2.bin");
    std::fs::write(&f1, vec![7u8; 400_000]).unwrap();
    std::fs::write(&f2, vec![9u8; 400_000]).unwrap();

    let id_a = Arc::new(DeviceIdentity::load_or_create(&dir_a, "node-a").unwrap());
    let id_b = Arc::new(DeviceIdentity::load_or_create(&dir_b, "node-b").unwrap());
    let trust_a = Arc::new(TrustStore::load(&dir_a).unwrap());
    let trust_b = Arc::new(TrustStore::load(&dir_b).unwrap());

    let h_a = Arc::new(AutoHandler::default());
    let h_b = Arc::new(AutoHandler::default());

    let engine_a = TransferEngine::start(
        engine_cfg(&dir_a, 8941),
        id_a,
        trust_a,
        h_a.clone() as Arc<dyn SessionHandler>,
    )
    .await
    .unwrap();
    let engine_b = TransferEngine::start(
        engine_cfg(&dir_b, 8942),
        id_b,
        trust_b,
        h_b.clone() as Arc<dyn SessionHandler>,
    )
    .await
    .unwrap();

    let addr_a: std::net::SocketAddr = format!("127.0.0.1:{}", engine_a.port).parse().unwrap();
    let sess_b = engine_b.connect(addr_a).await.unwrap();
    let _sess_a = wait_session(&h_a).await;

    let noop_sink: EventSink = Arc::new(|_| {});

    // 两个任务并发：各自 file_seq 都从 0 起，修复前互相覆盖导致串扰
    let s1 = sess_b.clone();
    let sink1 = noop_sink.clone();
    let t1 = tokio::spawn(async move {
        lt_transfer::send::send_files(
            s1,
            1,
            vec![make_item(&f1, "f1.bin")],
            sink1,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap()
    });
    let s2 = sess_b.clone();
    let t2 = tokio::spawn(async move {
        lt_transfer::send::send_files(
            s2,
            2,
            vec![make_item(&f2, "f2.bin")],
            noop_sink,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap()
    });

    let (r1, r2) = tokio::join!(t1, t2);
    let (r1, r2) = (r1.unwrap(), r2.unwrap());
    assert_eq!((r1.ok, r1.failed), (1, 0));
    assert_eq!((r2.ok, r2.failed), (1, 0));

    // 等接收端两条 Summary 都到
    for _ in 0..200 {
        if *h_a.summaries.lock().unwrap() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(*h_a.summaries.lock().unwrap(), 2);

    // 内容正确落盘（串扰会导致写入失败而非两个 400_000 字节的完整文件）
    assert_eq!(
        std::fs::read(dir_a.join("received/f1.bin")).unwrap().len(),
        400_000
    );
    assert_eq!(
        std::fs::read(dir_a.join("received/f2.bin")).unwrap().len(),
        400_000
    );

    engine_a.shutdown();
    engine_b.shutdown();
    let _ = std::fs::remove_dir_all(&root);
}
