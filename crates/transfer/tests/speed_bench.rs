use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crypto::trust::TrustStore;
use crypto::DeviceIdentity;
use transfer::engine::{EngineConfig, TransferEngine};
use transfer::session::{
    EngineEvent, EventSink, IncomingTransfer, Session, SessionConfig, SessionHandler, SessionInfo,
};

#[derive(Default)]
struct AutoHandler {
    session: Mutex<Option<Arc<Session>>>,
    summary: Mutex<Option<(u32, u32)>>,
}

impl SessionHandler for AutoHandler {
    fn connected(&self, session: Arc<Session>, _info: SessionInfo) {
        *self.session.lock().unwrap() = Some(session);
    }
    fn pair_needed(
        &self,
        session: Arc<Session>,
        pair_id: u64,
        _info: SessionInfo,
        _code: String,
        _is_initiator: bool,
    ) {
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
        force_tcp: true,
    }
}

fn make_item(path: &std::path::Path, name: &str) -> file::traverse::TransferItem {
    let meta = std::fs::metadata(path).unwrap();
    file::traverse::TransferItem {
        abs_path: path.to_path_buf(),
        rel_path: name.to_string(),
        size: meta.len(),
        mtime_unix: 0,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bench_quic_transfer_speed() {
    let root = std::env::temp_dir().join(format!("bt_speed_{}", std::process::id()));
    let dir_a = root.join("a");
    let dir_b = root.join("b");
    let src = root.join("src");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();
    std::fs::create_dir_all(&src).unwrap();

    let test_file = src.join("100mb.bin");
    const FILE_SIZE: usize = 100 * 1024 * 1024; // 100 MB
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&test_file).unwrap();
        let chunk = vec![0x5au8; 1024 * 1024]; // 1MB 块
        for _ in 0..100 {
            f.write_all(&chunk).unwrap();
        }
        f.sync_all().unwrap();
    }

    let id_a = Arc::new(DeviceIdentity::load_or_create(&dir_a, "node-a").unwrap());
    let id_b = Arc::new(DeviceIdentity::load_or_create(&dir_b, "node-b").unwrap());
    let trust_a = Arc::new(TrustStore::load(&dir_a).unwrap());
    let trust_b = Arc::new(TrustStore::load(&dir_b).unwrap());

    let h_a = Arc::new(AutoHandler::default());
    let h_b = Arc::new(AutoHandler::default());

    let engine_a = TransferEngine::start(
        engine_cfg(&dir_a, 9101),
        id_a,
        trust_a,
        h_a.clone() as Arc<dyn SessionHandler>,
    )
    .await
    .unwrap();

    let engine_b = TransferEngine::start(
        engine_cfg(&dir_b, 9102),
        id_b,
        trust_b,
        h_b.clone() as Arc<dyn SessionHandler>,
    )
    .await
    .unwrap();

    let addr_a: std::net::SocketAddr = format!("127.0.0.1:{}", engine_a.port).parse().unwrap();
    let sess_b = engine_b.connect(addr_a).await.unwrap();
    assert_eq!(sess_b.transport(), transfer::TransportKind::Tcp);

    let noop_sink: EventSink = Arc::new(|_| {});
    let cancel = Arc::new(AtomicBool::new(false));

    println!("\n=== 开始 100MB QUIC 本地传输压测 ===");
    let start_time = Instant::now();

    let sum = transfer::send::send_files(
        sess_b.clone(),
        1,
        [1u8; 16],
        vec![make_item(&test_file, "100mb.bin")],
        noop_sink.clone(),
        cancel.clone(),
    )
    .await
    .unwrap();

    let elapsed = start_time.elapsed();
    let elapsed_sec = elapsed.as_secs_f64();
    let mb_per_sec = (FILE_SIZE as f64 / (1024.0 * 1024.0)) / elapsed_sec;

    println!(">>> 传输结果: ok={}, failed={}", sum.ok, sum.failed);
    println!(">>> 传输耗时: {:.3} 秒", elapsed_sec);
    println!(">>> 实际吞吐速率: {:.2} MB/s", mb_per_sec);
    println!("=======================================\n");

    assert_eq!((sum.ok, sum.failed), (1, 0));
    assert!(mb_per_sec > 50.0, "本地传输吞吐应远大于 50MB/s，实际为 {:.2} MB/s", mb_per_sec);

    let _ = std::fs::remove_dir_all(&root);
}
