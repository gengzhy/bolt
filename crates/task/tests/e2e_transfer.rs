use std::time::{Duration, Instant};
use task::App;

#[test]
fn test_two_windows_apps_transfer_and_resume() {
    crypto::ensure_provider();

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
fn test_two_windows_apps_cancel_task() {
    crypto::ensure_provider();

    let temp_root = std::env::temp_dir().join(format!("lt_e2e_cancel_{}_{}", std::process::id(), fastrand()));
    let dir_a = temp_root.join("client_a");
    let dir_b = temp_root.join("client_b");
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    // 1. 创建源测试大文件（32MB）
    let src_file = dir_a.join("test_32mb.dat");
    let mut data = vec![0u8; 32 * 1024 * 1024];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = (i % 251) as u8;
    }
    std::fs::write(&src_file, &data).unwrap();

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

    // 5. 等待传输进行中后触发取消
    let wait_start = Instant::now();
    loop {
        let tasks_a: Vec<serde_json::Value> = serde_json::from_str(&app_a.get_tasks_json()).unwrap();
        let done = tasks_a.first().and_then(|t| t["done_bytes"].as_u64()).unwrap_or(0);
        if done >= 1024 * 1024 || wait_start.elapsed() > Duration::from_secs(5) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = app_a.cancel_task(task_id_a);
    std::thread::sleep(Duration::from_millis(500));

    // 6. 验证取消后状态
    let tasks_a: Vec<serde_json::Value> = serde_json::from_str(&app_a.get_tasks_json()).unwrap();
    let state_a = tasks_a[0]["state"].as_str().unwrap();
    assert_eq!(state_a, "cancelled");

    app_a.shutdown();
    app_b.shutdown();
    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn test_multi_files_batch_transfer() {
    crypto::ensure_provider();

    let temp_root = std::env::temp_dir().join(format!("lt_e2e_multi_{}_{}", std::process::id(), fastrand()));
    let dir_a = temp_root.join("client_a");
    let dir_b = temp_root.join("client_b");
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    // 1. 创建 5 个混合大小的测试文件，总计约 16MB
    let file_sizes = [
        ("file_1_small.dat", 256 * 1024),         // 256KB
        ("file_2_medium.dat", 2 * 1024 * 1024),    // 2MB
        ("file_3_large.dat", 8 * 1024 * 1024),     // 8MB
        ("file_4_medium.dat", 2 * 1024 * 1024),    // 2MB
        ("file_5_small.dat", 512 * 1024),         // 512KB
    ];

    let mut send_paths = Vec::new();
    let mut expected_hashes = std::collections::HashMap::new();

    for (idx, (name, size)) in file_sizes.iter().enumerate() {
        let path = dir_a.join(name);
        let mut data = vec![0u8; *size];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = ((i * (idx + 1) + 13) % 251) as u8;
        }
        std::fs::write(&path, &data).unwrap();
        let hash = blake3::hash(&data);
        expected_hashes.insert(*name, hash);
        send_paths.push(path.to_str().unwrap().to_string());
    }

    // 2. 启动双端
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

    // 4. A 批量发送 5 个文件
    let task_id_a = app_a.send_files(&uuid_b, &send_paths).expect("send_files");
    println!("Started multi-file task {} on Client A", task_id_a);

    // 5. 等待全部文件传输完成
    let start = Instant::now();
    let mut success = false;
    while start.elapsed() < Duration::from_secs(30) {
        let tasks_a_json = app_a.get_tasks_json();
        let tasks_b_json = app_b.get_tasks_json();

        if tasks_a_json.contains("\"state\":\"done\"") && tasks_b_json.contains("\"state\":\"done\"") {
            success = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(success, "Multi-file transfer timed out! Tasks A: {}, Tasks B: {}", app_a.get_tasks_json(), app_b.get_tasks_json());

    // 6. 验证落盘文件哈希
    for (name, _) in &file_sizes {
        let dest_file = save_dir.join(name);
        assert!(dest_file.exists(), "Received file {:?} does not exist!", dest_file);
        let content = std::fs::read(&dest_file).unwrap();
        let actual_hash = blake3::hash(&content);
        let expected = expected_hashes.get(*name).unwrap();
        assert_eq!(&actual_hash, expected, "File {} content corrupted!", name);
    }
    println!("All 5 files verified with BLAKE3 checksum: 100% PASS!");

    app_a.shutdown();
    app_b.shutdown();
    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn test_auto_rename_on_collision_and_smooth_progress() {
    crypto::ensure_provider();

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
