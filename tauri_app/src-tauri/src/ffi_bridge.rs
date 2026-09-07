//! lt-ffi 桥接层：Tauri 命令 + FFI 事件 → 前端事件转发。
//!
//! - 初始化在 `setup` 中完成（数据目录取应用私有目录）。
//! - 事件回调运行在 lt-ffi 专用分发线程，仅做拷贝 + `Emitter::emit`
//!   （tauri 事件通道线程安全），前端在 `lt://event` 上监听。

use std::ffi::{c_char, c_int, CStr, CString};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};

/// 全局 AppHandle（事件转发用）。lt-ffi 单实例，进程生命周期内有效。
static APP: Mutex<Option<AppHandle>> = Mutex::new(None);

// 全局实例锁句柄，进程生命周期内保持独占
static INSTANCE_LOCK: Mutex<Option<std::fs::File>> = Mutex::new(None);

fn get_isolated_data_dir(base_dir: std::path::PathBuf) -> std::path::PathBuf {
    // 优先检查命令行参数中是否显式指定了 --data-dir
    for arg in std::env::args() {
        if let Some(rest) = arg.strip_prefix("--data-dir=") {
            return std::path::PathBuf::from(rest);
        }
    }
    std::fs::create_dir_all(&base_dir).ok();

    // 在 Windows 下通过独占锁检测已有实例，实现本地多开测试时的完美隔离
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let lock_path = base_dir.join(".instance.lock");
        // share_mode(0) 表示禁止其他进程共享读写，主实例持有该句柄直到进程退出
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .share_mode(0)
            .open(&lock_path)
        {
            Ok(file) => {
                *INSTANCE_LOCK.lock().unwrap() = Some(file);
                base_dir
            }
            Err(_) => {
                // 主目录已被首个客户端实例占用：自动为第 2 实例分配独立的持久化目录
                let parent = base_dir.parent().unwrap_or(&base_dir);
                let inst2_dir = parent.join("com.lt.desktop_inst2");
                std::fs::create_dir_all(&inst2_dir).ok();
                let lock_path2 = inst2_dir.join(".instance.lock");
                if let Ok(file2) = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .share_mode(0)
                    .open(&lock_path2)
                {
                    *INSTANCE_LOCK.lock().unwrap() = Some(file2);
                }
                inst2_dir
            }
        }
    }
    #[cfg(not(windows))]
    {
        base_dir
    }
}

pub fn init(app: AppHandle) -> Result<(), String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    let isolated_dir = get_isolated_data_dir(data_dir);
    let data_dir_str = isolated_dir.to_string_lossy().to_string();
    let code = lt_ffi::lt_init(cstring(&data_dir_str).as_ptr());
    if code != 0 {
        return Err(format!("lt_init failed: {code}"));
    }
    lt_ffi::lt_set_event_callback(Some(forward_event));
    // 启动即对外广播/探测（与 Android 前台服务语义对齐，后台也能被发现）
    lt_ffi::lt_start_discovery();
    *APP.lock().unwrap() = Some(app);
    Ok(())
}

/// FFI 事件回调：拷贝 payload，转发给前端。
extern "C" fn forward_event(event_id: c_int, payload_json: *const c_char) {
    let payload = if payload_json.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(payload_json) }
            .to_string_lossy()
            .into_owned()
    };
    let guard = APP.lock().unwrap();
    if let Some(app) = guard.as_ref() {
        let _ = app.emit(
            "lt://event",
            serde_json::json!({ "id": event_id, "payload": payload }),
        );
    }
}

// ---------------- 工具 ----------------

fn cstring(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

/// 取回 FFI 字符串并释放。
fn take_string(p: *mut c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    lt_ffi::lt_free_string(p);
    s
}

/// 取回 FFI JSON 字符串并解析为 serde_json::Value。
fn take_json(p: *mut c_char) -> serde_json::Value {
    let s = take_string(p);
    serde_json::from_str(&s).unwrap_or(serde_json::Value::Null)
}

// ---------------- 查询类命令 ----------------

#[tauri::command]
pub fn get_devices() -> serde_json::Value {
    take_json(lt_ffi::lt_get_devices())
}

#[tauri::command]
pub fn get_tasks() -> serde_json::Value {
    take_json(lt_ffi::lt_get_tasks())
}

#[tauri::command]
pub fn get_config() -> serde_json::Value {
    take_json(lt_ffi::lt_get_config())
}

#[tauri::command]
pub fn get_local_fingerprint() -> String {
    take_string(lt_ffi::lt_get_local_fingerprint())
}

#[tauri::command]
pub fn get_local_info() -> serde_json::Value {
    take_json(lt_ffi::lt_get_local_info())
}

#[tauri::command]
pub fn version() -> String {
    if lt_ffi::lt_version().is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(lt_ffi::lt_version()) }
        .to_string_lossy()
        .into_owned()
}

// ---------------- 操作类命令 ----------------

#[tauri::command]
pub fn set_config(json: String) -> Result<(), i32> {
    let code = lt_ffi::lt_set_config(cstring(&json).as_ptr());
    if code == 0 { Ok(()) } else { Err(code) }
}

#[tauri::command]
pub fn start_discovery() {
    lt_ffi::lt_start_discovery();
}

#[tauri::command]
pub fn stop_discovery() {
    lt_ffi::lt_stop_discovery();
}

#[tauri::command]
pub fn probe_network() {
    lt_ffi::lt_probe_network();
}

#[tauri::command]
pub fn add_manual_device(ip: String, port: u16) -> Result<(), i32> {
    let code = lt_ffi::lt_add_manual_device(cstring(&ip).as_ptr(), port);
    if code == 0 { Ok(()) } else { Err(code) }
}

#[tauri::command]
pub fn connect(uuid: String) -> Result<(), i32> {
    let code = lt_ffi::lt_connect(cstring(&uuid).as_ptr());
    if code == 0 { Ok(()) } else { Err(code) }
}

#[tauri::command]
pub fn connect_addr(ip: String, port: u16) -> Result<(), i32> {
    let code = lt_ffi::lt_connect_addr(cstring(&ip).as_ptr(), port);
    if code == 0 { Ok(()) } else { Err(code) }
}

#[tauri::command]
pub fn disconnect(uuid: String) -> Result<(), i32> {
    let code = lt_ffi::lt_disconnect(cstring(&uuid).as_ptr());
    if code == 0 { Ok(()) } else { Err(code) }
}

#[tauri::command]
pub fn respond_pair(pair_id: u64, accept: bool) {
    lt_ffi::lt_respond_pair(pair_id, accept as c_int);
}

#[tauri::command]
pub fn respond_transfer(req_id: u64, accept: bool) {
    lt_ffi::lt_respond_transfer(req_id, accept as c_int);
}

#[tauri::command]
pub fn send_files(uuid: String, paths: Vec<String>) -> Result<u64, i32> {
    let paths_json = serde_json::to_string(&paths).map_err(|_| -1)?;
    let mut task_id: u64 = 0;
    let code = lt_ffi::lt_send_files(
        cstring(&uuid).as_ptr(),
        cstring(&paths_json).as_ptr(),
        &mut task_id,
    );
    if code == 0 { Ok(task_id) } else { Err(code) }
}

#[tauri::command]
pub fn pause_task(task_id: u64) -> Result<(), i32> {
    let code = lt_ffi::lt_pause_task(task_id);
    if code == 0 { Ok(()) } else { Err(code) }
}

#[tauri::command]
pub fn resume_task(task_id: u64) -> Result<(), i32> {
    let code = lt_ffi::lt_resume_task(task_id);
    if code == 0 { Ok(()) } else { Err(code) }
}

#[tauri::command]
pub fn cancel_task(task_id: u64) -> Result<(), i32> {
    let code = lt_ffi::lt_cancel_task(task_id);
    if code == 0 { Ok(()) } else { Err(code) }
}

#[tauri::command]
pub fn clear_records() {
    lt_ffi::lt_clear_records();
}

#[tauri::command]
pub fn clear_temp_cache() -> Result<(), i32> {
    let code = lt_ffi::lt_clear_temp_cache();
    if code == 0 { Ok(()) } else { Err(code) }
}

/// 在资源管理器中定位并高亮显示接收到的文件（接收任务「打开文件夹」）。
/// 1. 若目标路径存在且为文件，则使用 explorer /select,"path" 精确高亮选中；
/// 2. 若目标路径存在且为目录，则直接打开该目录（严禁携带 /select，避免 Windows 异常回退至文档目录）；
/// 3. 若目标路径不存在（例如自动重命名修改了文件名），优先尝试打开其有效的父目录；
/// 4. 若父目录仍不存在，从当前引擎配置获取 save_dir；若仍无则回落到用户系统 Downloads 目录。
#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    let target = std::path::PathBuf::from(&path);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;

        // 1. 文件存在且为普通文件：高亮选中（Windows 规范：/select,"path"，引号仅包裹路径本身）
        if target.is_file() {
            let mut cmd = std::process::Command::new("explorer");
            cmd.raw_arg(format!("/select,\"{}\"", target.display()));
            cmd.spawn().map_err(|e| e.to_string())?;
            return Ok(());
        }

        // 2. 确定要直接打开的目标目录
        let dir_to_open = if target.is_dir() {
            Some(target)
        } else if let Some(parent) = target.parent().filter(|p| p.is_dir()) {
            Some(parent.to_path_buf())
        } else {
            // 从配置读取 save_dir
            let cfg_val = get_config();
            if let Some(s) = cfg_val.get("save_dir").and_then(|v| v.as_str()) {
                let p = std::path::PathBuf::from(s);
                if p.is_dir() {
                    Some(p)
                } else {
                    None
                }
            } else {
                None
            }
        };

        // 3. 回退系统下载目录：%USERPROFILE%\Downloads
        let fallback_dir = dir_to_open.or_else(|| {
            std::env::var_os("USERPROFILE").map(|u| std::path::PathBuf::from(u).join("Downloads"))
        });

        if let Some(dir) = fallback_dir {
            let mut cmd = std::process::Command::new("explorer");
            cmd.arg(&dir);
            cmd.spawn().map_err(|e| e.to_string())?;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let to_open = if target.exists() {
            target
        } else if let Some(parent) = target.parent().filter(|p| p.exists()) {
            parent.to_path_buf()
        } else {
            std::path::PathBuf::from(".")
        };
        let _ = std::process::Command::new("xdg-open")
            .arg(to_open)
            .spawn();
    }

    Ok(())
}

/// 统计一组待发送路径：每项返回 `{path, name, is_dir, size, files}`。
/// 目录递归遍历（与真正发送时同一遍历器），不可访问的项 `error=true`。
#[tauri::command]
pub fn inspect_paths(paths: Vec<String>) -> serde_json::Value {
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        let pb = std::path::PathBuf::from(&p);
        let is_dir = pb.is_dir();
        let name = pb
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| p.clone());
        let entry = match lt_file::traverse::traverse(std::slice::from_ref(&pb)) {
            Ok(res) => serde_json::json!({
                "path": p,
                "name": name,
                "is_dir": is_dir,
                "size": res.total_size,
                "files": res.items.len(),
            }),
            Err(_) => serde_json::json!({
                "path": p,
                "name": name,
                "is_dir": is_dir,
                "size": 0u64,
                "files": 0usize,
                "error": true,
            }),
        };
        out.push(entry);
    }
    serde_json::Value::Array(out)
}
