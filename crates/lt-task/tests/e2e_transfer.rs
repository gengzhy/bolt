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

    // 1. 创建源测试大文件（96MB）
    let src_file = dir_a.join("test_96mb.dat");
    let mut data = vec![0u8; 96 * 1024 * 1024];
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

    // 5. 等待传输进行中（至少 5MB）再触发暂停
    let wait_start = Instant::now();
    loop {
        let tasks_a: Vec<serde_json::Value> = serde_json::from_str(&app_a.get_tasks_json()).unwrap();
        let done = tasks_a.first().and_then(|t| t["done_bytes"].as_u64()).unwrap_or(0);
        if done >= 5 * 1024 * 1024 || wait_start.elapsed() > Duration::from_secs(5) {
            println!("Pausing at done_bytes = {}", done);
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = app_a.pause_task(task_id_a);
    println!("Cancelled task {} on Client A", task_id_a);
    std::thread::sleep(Duration::from_millis(500));

    // 5.1 验证取消后状态
    let tasks_a: Vec<serde_json::Value> = serde_json::from_str(&app_a.get_tasks_json()).unwrap();
    let tasks_b: Vec<serde_json::Value> = serde_json::from_str(&app_b.get_tasks_json()).unwrap();
    let state_a = tasks_a[0]["state"].as_str().unwrap();
    println!("Cancelled status: Client A state = {}, Client B state = {}", state_a, tasks_b[0]["state"].as_str().unwrap());
    assert_eq!(state_a, "cancelled");

    // 获取 task_uid
    let uid_before_resume = tasks_a[0]["task_uid"].as_str().unwrap().to_string();
    println!("Task UID before retry: {}", uid_before_resume);

    // 6. 重试传输
    app_a.resume_task(task_id_a).expect("resume_task");
    println!("Retried task {} on Client A", task_id_a);

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

#[test]
fn test_two_windows_apps_disconnect_and_autoreconnect_resume() {
    lt_crypto::ensure_provider();

    let temp_root = std::env::temp_dir().join(format!("lt_e2e_recon_{}_{}", std::process::id(), fastrand()));
    let dir_a = temp_root.join("client_a");
    let dir_b = temp_root.join("client_b");
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    // 1. 创建源测试大文件（8MB）
    let src_file = dir_a.join("test_recon_8mb.dat");
    let mut data = vec![0u8; 8 * 1024 * 1024];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = ((i * 3 + 17) % 251) as u8;
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
    let task_id_a = app_a.send_files(&uuid_b, &[src_path_str.clone()]).expect("send_files");

    // 5. 传输中途强行断开连接（模拟移动端熄屏断网）
    std::thread::sleep(Duration::from_millis(60));
    let _ = app_a.disconnect(&uuid_b);
    println!("Severed connection mid-transfer for task {}", task_id_a);
    std::thread::sleep(Duration::from_millis(600));

    // 6. 重新连接，触发断线自动重连续传机制
    println!("Reconnecting A to B to trigger auto-resume...");
    app_a.connect_addr("127.0.0.1", port_b).expect("reconnect");

    // 7. 等待传输恢复并完成
    let start = Instant::now();
    let mut success = false;
    while start.elapsed() < Duration::from_secs(25) {
        let tasks_a_json = app_a.get_tasks_json();
        let tasks_b_json = app_b.get_tasks_json();

        if tasks_a_json.contains("\"state\":\"done\"") && tasks_b_json.contains("\"state\":\"done\"") {
            success = true;
            println!("Disconnect-and-reconnect transfer completed successfully!");
            println!("Tasks A: {}", tasks_a_json);
            println!("Tasks B: {}", tasks_b_json);
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    assert!(success, "Disconnect-reconnect transfer timed out! A: {}, B: {}", app_a.get_tasks_json(), app_b.get_tasks_json());

    // 8. 验证两端落盘哈希
    let received_files: Vec<_> = std::fs::read_dir(&save_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(!received_files.is_empty(), "Received file not found in {:?}", save_dir);
    let recv_content = std::fs::read(received_files[0].path()).unwrap();
    let recv_hash = blake3::hash(&recv_content);
    assert_eq!(recv_hash, expected_hash, "Reconnected transfer file content corrupted!");
    println!("Reconnected transfer hash verified: OK!");

    app_a.shutdown();
    app_b.shutdown();
    let _ = std::fs::remove_dir_all(&temp_root);
}

/// 【核心复杂场景高强度端到端测试】：
/// 验证 5 个混合大小文件并发批量传输、中途暂停时两端进度 100% 精确对齐、
/// 续传时已完成文件秒级跳过与未完成文件断点续传、最终全量文件 BLAKE3 哈希比对。
#[test]
fn test_multi_files_batch_pause_and_resume() {
    lt_crypto::ensure_provider();

    let temp_root = std::env::temp_dir().join(format!("lt_e2e_multi_{}_{}", std::process::id(), fastrand()));
    let dir_a = temp_root.join("client_a");
    let dir_b = temp_root.join("client_b");
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    // 1. 创建 5 个混合大小的测试文件，总计约 28MB
    let file_sizes = [
        ("file_1_small.dat", 512 * 1024),         // 512KB
        ("file_2_medium.dat", 4 * 1024 * 1024),    // 4MB
        ("file_3_large.dat", 12 * 1024 * 1024),    // 12MB
        ("file_4_medium.dat", 3 * 1024 * 1024),    // 3MB
        ("file_5_large.dat", 8 * 1024 * 1024),     // 8MB
    ];

    let mut src_paths = Vec::new();
    let mut expected_hashes = std::collections::HashMap::new();
    let mut expected_total_size = 0u64;

    for (name, size) in &file_sizes {
        let path = dir_a.join(name);
        let mut data = vec![0u8; *size];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = ((i * 7 + 13) % 251) as u8;
        }
        std::fs::write(&path, &data).unwrap();
        expected_hashes.insert(name.to_string(), blake3::hash(&data));
        src_paths.push(path.to_str().unwrap().to_string());
        expected_total_size += *size as u64;
    }

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
    app_b.set_config(&format!(r#"{{"auto_accept_trusted": true, "collision": "overwrite", "save_dir": {}}}"#, save_dir_json)).unwrap();

    // 3. A 直连 B
    app_a.connect_addr("127.0.0.1", port_b).expect("connect_addr");
    std::thread::sleep(Duration::from_millis(300));

    // 4. A 批量发送 5 个文件到 B
    let task_id_a = app_a.send_files(&uuid_b, &src_paths).expect("send_files");
    println!("Started multi-file task {} on Client A, total_size = {}", task_id_a, expected_total_size);

    // 5. 等待传输进行中（至少 3MB 数据已在传输）后触发暂停
    let wait_start = Instant::now();
    loop {
        let tasks_a: Vec<serde_json::Value> = serde_json::from_str(&app_a.get_tasks_json()).unwrap();
        let done = tasks_a.first().and_then(|t| t["done_bytes"].as_u64()).unwrap_or(0);
        if done >= 3 * 1024 * 1024 || wait_start.elapsed() > Duration::from_secs(5) {
            println!("Pausing multi-file task at done_bytes = {}", done);
            break;
        }
        std::thread::sleep(Duration::from_millis(30));
    }

    let _ = app_a.pause_task(task_id_a);
    println!("Cancelled multi-file task {} on Client A", task_id_a);
    std::thread::sleep(Duration::from_millis(500));

    // 6. 验证取消状态
    let tasks_a: Vec<serde_json::Value> = serde_json::from_str(&app_a.get_tasks_json()).unwrap();
    let tasks_b: Vec<serde_json::Value> = serde_json::from_str(&app_b.get_tasks_json()).unwrap();
    assert_eq!(tasks_a.len(), 1, "Task A must have 1 task entry");
    assert_eq!(tasks_b.len(), 1, "Task B must have 1 task entry");
    assert_eq!(tasks_a[0]["state"].as_str().unwrap(), "cancelled");

    let uid_before_resume = tasks_a[0]["task_uid"].as_str().unwrap().to_string();

    // 7. 重试多文件传输
    app_a.resume_task(task_id_a).expect("resume_task");
    println!("Retried multi-file task {} on Client A", task_id_a);

    // 8. 等待全部 5 个文件传输完成
    let start = Instant::now();
    let mut success = false;
    while start.elapsed() < Duration::from_secs(30) {
        let tasks_a_json = app_a.get_tasks_json();
        let tasks_b_json = app_b.get_tasks_json();

        if tasks_a_json.contains("\"state\":\"done\"") && tasks_b_json.contains("\"state\":\"done\"") {
            success = true;
            println!("Multi-file resume transfer completed successfully on both ends!");
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(success, "Multi-file transfer timed out! Tasks A: {}, Tasks B: {}", app_a.get_tasks_json(), app_b.get_tasks_json());

    // 9. 【核心验证 2：两端任务状态、总字节数、task_uid 恒定一致】
    let tasks_a: Vec<serde_json::Value> = serde_json::from_str(&app_a.get_tasks_json()).unwrap();
    let tasks_b: Vec<serde_json::Value> = serde_json::from_str(&app_b.get_tasks_json()).unwrap();
    println!("Final Tasks A: {:?}", tasks_a);
    println!("Final Tasks B: {:?}", tasks_b);
    let uid_a = tasks_a[0]["task_uid"].as_str().unwrap();
    let uid_b = tasks_b[0]["task_uid"].as_str().unwrap();
    assert_eq!(uid_a, uid_before_resume, "task_uid must stay constant across multi-file resume!");
    assert_eq!(uid_a, uid_b, "task_uid on Sender and Receiver must match across resume!");
    assert_eq!(tasks_a[0]["done_bytes"].as_u64().unwrap(), expected_total_size);
    assert_eq!(tasks_b[0]["done_bytes"].as_u64().unwrap(), expected_total_size);

    let entries: Vec<_> = std::fs::read_dir(&save_dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    println!("Actual files in save_dir: {:?}", entries);
    for (name, _) in &file_sizes {
        let dest_file = save_dir.join(name);
        assert!(dest_file.exists(), "Received file {:?} does not exist! Actual files: {:?}", dest_file, entries);
        let content = std::fs::read(&dest_file).unwrap();
        let actual_hash = blake3::hash(&content);
        let expected = expected_hashes.get(*name).unwrap();
        assert_eq!(&actual_hash, expected, "File {} content corrupted after resume!", name);
    }
    println!("All 5 files verified with BLAKE3 checksum: 100% PASS!");

    app_a.shutdown();
    app_b.shutdown();
    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn test_auto_rename_on_collision_and_smooth_progress() {
    lt_crypto::ensure_provider();

    let temp_root = std::env::temp_dir().join(format!("lt_e2e_collision_{}_{}", std::process::id(), fastrand()));
    let dir_a = temp_root.join("client_a");
    let dir_b = temp_root.join("client_b");
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    let save_dir = dir_b.join("save");
    std::fs::create_dir_all(&save_dir).unwrap();

    // 1. 在接收端预先放置一个同名文件，内容为 "Receiver existing content"
    let existing_file = save_dir.join("report.docx");
    let existing_content = b"Receiver existing content with exact size match padding 1234567890";
    std::fs::write(&existing_file, existing_content).unwrap();

    // 2. 发送端创建同名文件，但内容为全新传输内容（确保同名不同内容）
    let src_file = dir_a.join("report.docx");
    let new_content = b"Sender brand new transmitted content padding xyz0987654321ABCDEFG";
    std::fs::write(&src_file, new_content).unwrap();
    let expected_new_hash = blake3::hash(new_content);

    // 3. 启动双端
    let app_a = App::init(Some(dir_a.clone())).expect("init A");
    let app_b = App::init(Some(dir_b.clone())).expect("init B");

    let uuid_a = app_a.device_uuid();
    let uuid_b = app_b.device_uuid();
    let fp_a = app_a.local_fingerprint();
    let fp_b = app_b.local_fingerprint();
    let port_b = app_b.engine_port();

    let save_dir_json = serde_json::to_string(save_dir.to_str().unwrap()).unwrap();
    app_a.trust().add(&uuid_b, &fp_b, "ClientB").unwrap();
    app_b.trust().add(&uuid_a, &fp_a, "ClientA").unwrap();
    app_b.set_config(&format!(r#"{{"auto_accept_trusted": true, "save_dir": {}, "collision": "rename"}}"#, save_dir_json)).unwrap();

    // 4. A 连 B
    app_a.connect_addr("127.0.0.1", port_b).expect("connect_addr");
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

    // 5. A 发送同名文件
    let _task_id_a = app_a
        .send_files(&uuid_b, &[src_file.to_str().unwrap().to_string()])
        .expect("send_files");

    // 6. 等待传输完毕
    let start = Instant::now();
    let mut success = false;
    while start.elapsed() < Duration::from_secs(10) {
        let tasks_a_json = app_a.get_tasks_json();
        let tasks_b_json = app_b.get_tasks_json();

        if tasks_a_json.contains("\"state\":\"done\"") && tasks_b_json.contains("\"state\":\"done\"") {
            success = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(success, "Collision transfer timed out! Tasks A: {}, Tasks B: {}", app_a.get_tasks_json(), app_b.get_tasks_json());

    // 7. 【核心验证 Bug 4】：
    // 接收端原有的 report.docx 必须完好无损（内容仍为 existing_content）
    let existing_now = std::fs::read(&existing_file).unwrap();
    assert_eq!(&existing_now, existing_content, "Existing file was unexpectedly overwritten!");

    // 自动重命名的文件 report(1).docx 必须生成，且内容与新发送的文件 100% 吻合！
    let renamed_file = save_dir.join("report(1).docx");
    assert!(renamed_file.exists(), "Renamed file report(1).docx does not exist! Bug 4 regression!");
    let renamed_content = std::fs::read(&renamed_file).unwrap();
    assert_eq!(blake3::hash(&renamed_content), expected_new_hash, "Renamed file content does not match!");

    // 8. 两端终态 done_bytes 100% 对齐，且接收端 current_file 精准更新为重命名后的真实文件名
    let tasks_a: Vec<serde_json::Value> = serde_json::from_str(&app_a.get_tasks_json()).unwrap();
    let tasks_b: Vec<serde_json::Value> = serde_json::from_str(&app_b.get_tasks_json()).unwrap();
    assert_eq!(tasks_a[0]["done_bytes"].as_u64().unwrap(), new_content.len() as u64);
    assert_eq!(tasks_b[0]["done_bytes"].as_u64().unwrap(), new_content.len() as u64);
    assert_eq!(tasks_b[0]["current_file"].as_str().unwrap(), "report(1).docx");

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
