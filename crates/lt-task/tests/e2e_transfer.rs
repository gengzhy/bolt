use std::time::{Duration, Instant};
use lt_task::App;

#[test]
fn test_two_windows_apps_transfer_and_resume() {
    lt_crypto::ensure_provider();

    let temp_root = std::env::temp_dir().join(format!("lt_e2e_{}_{}", std::process::id(), fastrand()));
    let dir_a = temp_root.join("client_a");
    let dir_b = temp_root.join("client_b");
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    // 1. 创建源测试大文件（4MB）
    let src_file = dir_a.join("test_4mb.dat");
    let mut data = vec![0u8; 4 * 1024 * 1024];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = (i % 251) as u8;
    }
    std::fs::write(&src_file, &data).unwrap();
    let expected_hash = blake3::hash(&data);

    // 2. 启动 Windows 客户端 A 与 客户端 B
    let app_a = App::init(Some(dir_a.clone())).expect("init A");
    let app_b = App::init(Some(dir_b.clone())).expect("init B");

    let uuid_a = app_a.device_uuid();
    let uuid_b = app_b.device_uuid();
    let fp_a = app_a.local_fingerprint();
    let fp_b = app_b.local_fingerprint();
    let port_b = app_b.engine_port();

    println!("Client A: UUID={}, Port={}", uuid_a, app_a.engine_port());
    println!("Client B: UUID={}, Port={}", uuid_b, port_b);

    // 3. 双方建立信任配对 & B 开启信任设备免密自动接收并指定独立保存目录
    let save_dir = dir_b.join("save");
    std::fs::create_dir_all(&save_dir).unwrap();
    let save_dir_json = serde_json::to_string(save_dir.to_str().unwrap()).unwrap();
    app_a.trust().add(&uuid_b, &fp_b, "ClientB").unwrap();
    app_b.trust().add(&uuid_a, &fp_a, "ClientA").unwrap();
    app_b.set_config(&format!(r#"{{"auto_accept_trusted": true, "save_dir": {}}}"#, save_dir_json)).unwrap();

    // 4. A 直连 B
    app_a.connect_addr("127.0.0.1", port_b).expect("connect_addr");

    // 等待连接建立
    let start = Instant::now();
    loop {
        if app_a.get_config_json().contains(&uuid_b) || app_b.get_config_json().contains(&uuid_a) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        if start.elapsed() > Duration::from_secs(5) {
            break;
        }
    }

    // 5. A 发送文件到 B
    let src_path_str = src_file.to_str().unwrap().to_string();
    let task_id_a = app_a.send_files(&uuid_b, &[src_path_str]).expect("send_files");
    println!("Started task {} on Client A", task_id_a);

    // 6. 等待传输完成并验证
    let start = Instant::now();
    let mut success = false;
    while start.elapsed() < Duration::from_secs(15) {
        let tasks_a_json = app_a.get_tasks_json();
        let tasks_b_json = app_b.get_tasks_json();

        if tasks_a_json.contains("\"state\":\"done\"") && tasks_b_json.contains("\"state\":\"done\"") {
            success = true;
            println!("Transfer completed successfully on both ends!");
            println!("Tasks A: {}", tasks_a_json);
            println!("Tasks B: {}", tasks_b_json);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(success, "Transfer timed out! Tasks A: {}, Tasks B: {}", app_a.get_tasks_json(), app_b.get_tasks_json());

    // 7. 验证两端 task_uid 完全一致
    let tasks_a: Vec<serde_json::Value> = serde_json::from_str(&app_a.get_tasks_json()).unwrap();
    let tasks_b: Vec<serde_json::Value> = serde_json::from_str(&app_b.get_tasks_json()).unwrap();
    assert_eq!(tasks_a.len(), 1);
    assert_eq!(tasks_b.len(), 1);

    let uid_a = tasks_a[0]["task_uid"].as_str().unwrap();
    let uid_b = tasks_b[0]["task_uid"].as_str().unwrap();
    println!("Task UID on A: {}", uid_a);
    println!("Task UID on B: {}", uid_b);
    assert!(!uid_a.is_empty(), "task_uid must not be empty");
    assert_eq!(uid_a, uid_b, "task_uid on Sender and Receiver must be 100% identical!");

    // 8. 验证两端 done_bytes 与 total_size 严格一致
    let done_a = tasks_a[0]["done_bytes"].as_u64().unwrap();
    let done_b = tasks_b[0]["done_bytes"].as_u64().unwrap();
    let total_a = tasks_a[0]["total_size"].as_u64().unwrap();
    let total_b = tasks_b[0]["total_size"].as_u64().unwrap();
    assert_eq!(done_a, 4 * 1024 * 1024);
    assert_eq!(done_b, 4 * 1024 * 1024);
    assert_eq!(total_a, 4 * 1024 * 1024);
    assert_eq!(total_b, 4 * 1024 * 1024);

    // 9. 验证接收端落盘文件哈希是否与源文件 100% 吻合
    let save_dir = dir_b.join("save");
    let received_files: Vec<_> = std::fs::read_dir(&save_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(!received_files.is_empty(), "Received file not found in {:?}", save_dir);
    let recv_content = std::fs::read(received_files[0].path()).unwrap();
    let recv_hash = blake3::hash(&recv_content);
    assert_eq!(recv_hash, expected_hash, "Received file corrupted!");

    // 10. 清理资源并优雅关机
    app_a.shutdown();
    app_b.shutdown();
    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn test_two_windows_apps_pause_and_resume() {
    lt_crypto::ensure_provider();

    let temp_root = std::env::temp_dir().join(format!("lt_e2e_pause_{}_{}", std::process::id(), fastrand()));
    let dir_a = temp_root.join("client_a");
    let dir_b = temp_root.join("client_b");
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    // 1. 创建源测试大文件（12MB）
    let src_file = dir_a.join("test_12mb.dat");
    let mut data = vec![0u8; 12 * 1024 * 1024];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = (i % 251) as u8;
    }
    std::fs::write(&src_file, &data).unwrap();
    let expected_hash = blake3::hash(&data);

    // 2. 启动 Windows 客户端 A 与 客户端 B
    let app_a = App::init(Some(dir_a.clone())).expect("init A");
    let app_b = App::init(Some(dir_b.clone())).expect("init B");

    let uuid_a = app_a.device_uuid();
    let uuid_b = app_b.device_uuid();
    let fp_a = app_a.local_fingerprint();
    let fp_b = app_b.local_fingerprint();
    let port_b = app_b.engine_port();

    let save_dir = dir_b.join("save");
    std::fs::create_dir_all(&save_dir).unwrap();
    let save_dir_json = serde_json::to_string(save_dir.to_str().unwrap()).unwrap();
    app_a.trust().add(&uuid_b, &fp_b, "ClientB").unwrap();
    app_b.trust().add(&uuid_a, &fp_a, "ClientA").unwrap();
    app_b.set_config(&format!(r#"{{"auto_accept_trusted": true, "save_dir": {}}}"#, save_dir_json)).unwrap();

    // 3. A 直连 B
    app_a.connect_addr("127.0.0.1", port_b).expect("connect_addr");
    std::thread::sleep(Duration::from_millis(300));

    // 4. A 发送文件到 B
    let src_path_str = src_file.to_str().unwrap().to_string();
    let task_id_a = app_a.send_files(&uuid_b, &[src_path_str]).expect("send_files");

    // 5. 传输过程中触发暂停
    std::thread::sleep(Duration::from_millis(80));
    let _ = app_a.pause_task(task_id_a);
    println!("Paused task {} on Client A", task_id_a);
    std::thread::sleep(Duration::from_millis(500));

    // 获取暂停时的 task_uid
    let tasks_a: Vec<serde_json::Value> = serde_json::from_str(&app_a.get_tasks_json()).unwrap();
    let uid_before_resume = tasks_a[0]["task_uid"].as_str().unwrap().to_string();
    println!("Task UID before resume: {}", uid_before_resume);

    // 6. 恢复续传（断点续传）
    app_a.resume_task(task_id_a).expect("resume_task");
    println!("Resumed task {} on Client A", task_id_a);

    // 7. 等待传输完成
    let start = Instant::now();
    let mut success = false;
    while start.elapsed() < Duration::from_secs(20) {
        let tasks_a_json = app_a.get_tasks_json();
        let tasks_b_json = app_b.get_tasks_json();

        if tasks_a_json.contains("\"state\":\"done\"") && tasks_b_json.contains("\"state\":\"done\"") {
            success = true;
            println!("Resume transfer completed successfully!");
            println!("Tasks A: {}", tasks_a_json);
            println!("Tasks B: {}", tasks_b_json);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(success, "Resume transfer timed out! Tasks A: {}, Tasks B: {}", app_a.get_tasks_json(), app_b.get_tasks_json());

    // 8. 验证断点续传后两端 task_uid 保持严格一致且未被篡改
    let tasks_a: Vec<serde_json::Value> = serde_json::from_str(&app_a.get_tasks_json()).unwrap();
    let tasks_b: Vec<serde_json::Value> = serde_json::from_str(&app_b.get_tasks_json()).unwrap();
    let uid_a = tasks_a[0]["task_uid"].as_str().unwrap();
    let uid_b = tasks_b[0]["task_uid"].as_str().unwrap();
    assert_eq!(uid_a, uid_before_resume, "task_uid must stay constant across resume!");
    assert_eq!(uid_a, uid_b, "task_uid on Sender and Receiver must match across resume!");

    // 9. 验证落盘文件内容与哈希完全无误
    let received_files: Vec<_> = std::fs::read_dir(&save_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(!received_files.is_empty(), "Received file not found in {:?}", save_dir);
    let recv_content = std::fs::read(received_files[0].path()).unwrap();
    let recv_hash = blake3::hash(&recv_content);
    assert_eq!(recv_hash, expected_hash, "Resumed file content corrupted!");
    println!("Resumed file hash verified: OK!");

    app_a.shutdown();
    app_b.shutdown();
    let _ = std::fs::remove_dir_all(&temp_root);
}

fn fastrand() -> u32 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345)
}
