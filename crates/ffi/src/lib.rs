//! ffi：统一 C 风格 FFI（实施方案第十节）。
//!
//! 命名：全部 `bt_` 前缀；整型返回 `0` 成功 / 负错误码；
//! 字符串返回 `*mut c_char`，调用方用 [`bt_free_string`] 释放。
//!
//! 事件分发：应用事件先进入无界队列，由专用线程回调上层
//! （避免 tokio 工作线程直接穿透到 UI 线程引发竞态）。
//!
//! 头文件：build.rs 通过 cbindgen 生成 `include/bt_api.h`。
//!
//! 安全约定：所有 `bt_*` 函数对入参指针做 NULL 检查后再解引用，
//! 这是 C 互操作库的惯用形态，故整体放行相应 lint。
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crossbeam_channel::{unbounded, Sender};
use once_cell::sync::Lazy;

use task::{App, BtEvent};

#[cfg(target_os = "android")]
mod android_jni;

/// 事件回调：event_id 见 task::events::EVT_*，payload_json 为 UTF-8 JSON。
pub type BtEventCallback = extern "C" fn(event_id: c_int, payload_json: *const c_char);

static APP: Mutex<Option<Arc<App>>> = Mutex::new(None);
static EVENT_QUEUE: Lazy<Sender<(i32, String)>> = Lazy::new(|| {
    let (tx, rx) = unbounded::<(i32, String)>();
    std::thread::Builder::new()
        .name("bt-ffi-events".into())
        .spawn(move || {
            while let Ok((id, payload)) = rx.recv() {
                let cb = *EVENT_CB.lock().unwrap();
                if let Some(cb) = cb {
                    if let Ok(c) = CString::new(payload) {
                        cb(id, c.as_ptr());
                    }
                }
            }
        })
        .expect("spawn ffi event thread");
    tx
});
static EVENT_CB: Mutex<Option<BtEventCallback>> = Mutex::new(None);

fn code_of(e: &utils::BtError) -> c_int {
    e.code() as c_int
}

fn err_not_init() -> c_int {
    utils::BtError::Internal.code() as c_int
}

fn with_app<F, T>(f: F) -> T
where
    F: FnOnce(&Arc<App>) -> T,
    T: Default,
{
    let guard = APP.lock().unwrap();
    match guard.as_ref() {
        Some(app) => f(app),
        None => T::default(),
    }
}

fn to_c_string(s: String) -> *mut c_char {
    CString::new(s)
        .map(|c| c.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

unsafe fn str_from<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

fn install_event_sink(app: &Arc<App>) {
    app.set_event_sink(move |ev: BtEvent| {
        let _ = EVENT_QUEUE.send((ev.id(), ev.payload_json()));
    });
}

// ================= 生命周期 =================

/// 初始化 Bolt（引擎 + 发现）。`data_dir` 为 NULL 用系统默认目录。
/// 返回 0 成功 / 负错误码。
#[no_mangle]
pub extern "C" fn bt_init(data_dir: *const c_char) -> c_int {
    // 日志（仅本地，不含文件内容与密钥）
    static INIT_LOG: std::sync::Once = std::sync::Once::new();
    INIT_LOG.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .with_writer(std::io::stderr)
            .try_init();
    });

    let mut guard = APP.lock().unwrap();
    if guard.is_some() {
        return 0; // 幂等
    }
    let dir = unsafe { str_from(data_dir) }.map(PathBuf::from);
    match App::init(dir) {
        Ok(app) => {
            install_event_sink(&app);
            *guard = Some(app);
            0
        }
        Err(e) => code_of(&e),
    }
}

/// 关闭全部（停发现、停引擎）。幂等。
#[no_mangle]
pub extern "C" fn bt_shutdown() {
    let app = APP.lock().unwrap().take();
    if let Some(app) = app {
        app.shutdown();
    }
}

/// 版本字符串（静态，不需释放）。
#[no_mangle]
pub extern "C" fn bt_version() -> *const c_char {
    static V: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
    V.as_ptr() as *const c_char
}

// ================= 事件 =================

/// 注册事件回调（覆盖旧回调，传 NULL 清除）。回调在专用分发线程执行。
#[no_mangle]
pub extern "C" fn bt_set_event_callback(
    cb: Option<extern "C" fn(event_id: c_int, payload_json: *const c_char)>,
) {
    *EVENT_CB.lock().unwrap() = cb;
}

// ================= 配置 =================

/// 合并更新配置（JSON 片段）。
#[no_mangle]
pub extern "C" fn bt_set_config(json: *const c_char) -> c_int {
    let Some(json) = (unsafe { str_from(json) }) else {
        return utils::BtError::InvalidArgument.code() as c_int;
    };
    with_app(|app| match app.set_config(json) {
        Ok(()) => 0,
        Err(e) => code_of(&e),
    })
}

/// 读取全量配置（JSON）。调用方 [`bt_free_string`] 释放。
#[no_mangle]
pub extern "C" fn bt_get_config() -> *mut c_char {
    with_app(|app| to_c_string(app.get_config_json()))
}

/// 本机证书指纹（冒号分隔，可用于屏幕比对）。
#[no_mangle]
pub extern "C" fn bt_get_local_fingerprint() -> *mut c_char {
    with_app(|app| to_c_string(app.local_fingerprint()))
}

/// 本机设备信息 JSON（uuid/name/dt/qport/tport/ver/stealth）。
/// Android 侧由 Kotlin 取此信息注册 NSD 服务（Rust 在安卓不跑 mDNS）。
/// 调用方 [`bt_free_string`] 释放。
#[no_mangle]
pub extern "C" fn bt_get_local_info() -> *mut c_char {
    with_app(|app| to_c_string(app.local_info_json()))
}

// ================= 发现 =================

#[no_mangle]
pub extern "C" fn bt_start_discovery() -> c_int {
    with_app(|app| {
        app.restart_discovery();
        0
    })
}

#[no_mangle]
pub extern "C" fn bt_stop_discovery() -> c_int {
    with_app(|app| {
        app.stop_discovery();
        0
    })
}

#[no_mangle]
pub extern "C" fn bt_probe_network() -> c_int {
    with_app(|app| {
        app.probe_network();
        0
    })
}

/// 设备列表（JSON 数组）。
#[no_mangle]
pub extern "C" fn bt_get_devices() -> *mut c_char {
    with_app(|app| to_c_string(app.get_devices_json()))
}

/// 手动添加设备（直连场景）。
#[no_mangle]
pub extern "C" fn bt_add_manual_device(ip: *const c_char, port: u16) -> c_int {
    let Some(ip) = (unsafe { str_from(ip) }) else {
        return utils::BtError::InvalidArgument.code() as c_int;
    };
    with_app(|app| {
        app.add_manual_device(ip, port);
        0
    })
}

// ================= 连接 / 配对 =================

/// 连接设备（异步，结果经 EVT_CONN_STATE / EVT_ERROR）。
#[no_mangle]
pub extern "C" fn bt_connect(uuid: *const c_char) -> c_int {
    let Some(uuid) = (unsafe { str_from(uuid) }) else {
        return utils::BtError::InvalidArgument.code() as c_int;
    };
    let guard = APP.lock().unwrap();
    let Some(app) = guard.as_ref() else {
        return err_not_init();
    };
    match app.connect(uuid) {
        Ok(()) => 0,
        Err(e) => code_of(&e),
    }
}

/// 直连任意地址。
#[no_mangle]
pub extern "C" fn bt_connect_addr(ip: *const c_char, port: u16) -> c_int {
    let Some(ip) = (unsafe { str_from(ip) }) else {
        return utils::BtError::InvalidArgument.code() as c_int;
    };
    let guard = APP.lock().unwrap();
    let Some(app) = guard.as_ref() else {
        return err_not_init();
    };
    match app.connect_addr(ip, port) {
        Ok(()) => 0,
        Err(e) => code_of(&e),
    }
}

#[no_mangle]
pub extern "C" fn bt_disconnect(uuid: *const c_char) -> c_int {
    let Some(uuid) = (unsafe { str_from(uuid) }) else {
        return utils::BtError::InvalidArgument.code() as c_int;
    };
    with_app(|app| match app.disconnect(uuid) {
        Ok(()) => 0,
        Err(e) => code_of(&e),
    })
}

/// 配对应答（pair_id 来自 EVT_PAIR_REQUEST）。
#[no_mangle]
pub extern "C" fn bt_respond_pair(pair_id: u64, accept: c_int) -> c_int {
    with_app(|app| {
        app.respond_pair(pair_id, accept != 0);
        0
    })
}

/// 传输请求应答（req_id 来自 EVT_TRANSFER_REQUEST）。
#[no_mangle]
pub extern "C" fn bt_respond_transfer(req_id: u64, accept: c_int) -> c_int {
    with_app(|app| {
        app.respond_transfer(req_id, accept != 0);
        0
    })
}

// ================= 任务 =================

/// 发送文件（paths_json 为字符串数组的 JSON）。
/// 成功返回 0 且 `out_task_id` 写入任务 ID。
#[no_mangle]
pub extern "C" fn bt_send_files(
    uuid: *const c_char,
    paths_json: *const c_char,
    out_task_id: *mut u64,
) -> c_int {
    let Some(uuid) = (unsafe { str_from(uuid) }) else {
        return utils::BtError::InvalidArgument.code() as c_int;
    };
    let Some(paths_json) = (unsafe { str_from(paths_json) }) else {
        return utils::BtError::InvalidArgument.code() as c_int;
    };
    let paths: Vec<String> = match serde_json::from_str(paths_json) {
        Ok(p) => p,
        Err(_) => return utils::BtError::InvalidArgument.code() as c_int,
    };
    let guard = APP.lock().unwrap();
    let Some(app) = guard.as_ref() else {
        return err_not_init();
    };
    match app.send_files(uuid, &paths) {
        Ok(task_id) => {
            if !out_task_id.is_null() {
                unsafe { *out_task_id = task_id };
            }
            0
        }
        Err(e) => code_of(&e),
    }
}

#[no_mangle]
pub extern "C" fn bt_cancel_task(task_id: u64) -> c_int {
    with_app(|app| match app.cancel_task(task_id) {
        Ok(()) => 0,
        Err(e) => code_of(&e),
    })
}

/// 任务列表（JSON 数组）。
#[no_mangle]
pub extern "C" fn bt_get_tasks() -> *mut c_char {
    with_app(|app| to_c_string(app.get_tasks_json()))
}

/// 清除已结束的任务记录。
#[no_mangle]
pub extern "C" fn bt_clear_records() -> c_int {
    with_app(|app| {
        app.clear_records();
        0
    })
}

/// 清理临时缓存（临时文件 + 断点记录）。
#[no_mangle]
pub extern "C" fn bt_clear_temp_cache() -> c_int {
    with_app(|app| match app.clear_temp_cache() {
        Ok(()) => 0,
        Err(e) => code_of(&e),
    })
}

// ================= Android NSD 桥 =================

/// Kotlin NsdManager 发现结果注入（JSON）。
#[no_mangle]
pub extern "C" fn bt_nsd_inject_device(json: *const c_char) -> c_int {
    let Some(json) = (unsafe { str_from(json) }) else {
        return utils::BtError::InvalidArgument.code() as c_int;
    };
    with_app(|app| {
        if app.nsd().inject_json(json) {
            0
        } else {
            utils::BtError::InvalidArgument.code() as c_int
        }
    })
}

/// Kotlin 侧服务丢失。
#[no_mangle]
pub extern "C" fn bt_nsd_remove_device(uuid: *const c_char) -> c_int {
    let Some(uuid) = (unsafe { str_from(uuid) }) else {
        return utils::BtError::InvalidArgument.code() as c_int;
    };
    with_app(|app| {
        app.nsd().remove(uuid);
        0
    })
}

// ================= 内存 =================

/// 释放本库返回的字符串。
#[no_mangle]
pub extern "C" fn bt_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    extern "C" fn test_cb(id: c_int, payload: *const c_char) {
        LAST_EVENT.store(id, Ordering::SeqCst);
        let s = unsafe { CStr::from_ptr(payload) }
            .to_str()
            .unwrap()
            .to_string();
        LAST_PAYLOAD.lock().unwrap().push(s);
    }
    static LAST_EVENT: AtomicI32 = AtomicI32::new(0);
    static LAST_PAYLOAD: Mutex<Vec<String>> = Mutex::new(Vec::new());

    #[test]
    fn ffi_lifecycle() {
        let dir = std::env::temp_dir().join(format!("bt_ffi_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cdir = CString::new(dir.to_str().unwrap()).unwrap();

        bt_set_event_callback(Some(test_cb));
        assert_eq!(bt_init(cdir.as_ptr()), 0);
        assert_eq!(bt_init(cdir.as_ptr()), 0); // 幂等

        // 指纹与版本
        let fp = bt_get_local_fingerprint();
        assert!(!fp.is_null());
        bt_free_string(fp);
        assert!(!bt_version().is_null());

        // 配置
        let cfg = bt_get_config();
        assert!(!cfg.is_null());
        bt_free_string(cfg);
        let patch = CString::new(r#"{"device_name":"FFI测试"}"#).unwrap();
        assert_eq!(bt_set_config(patch.as_ptr()), 0);

        // 发现/设备
        assert_eq!(bt_start_discovery(), 0);
        let devs = bt_get_devices();
        assert!(!devs.is_null());
        bt_free_string(devs);
        assert_eq!(bt_probe_network(), 0);
        assert_eq!(bt_stop_discovery(), 0);

        // 任务列表
        let tasks = bt_get_tasks();
        assert!(!tasks.is_null());
        bt_free_string(tasks);
        assert_eq!(bt_clear_records(), 0);
        assert_eq!(bt_clear_temp_cache(), 0);

        // NSD 注入
        let j = CString::new(r#"{"uuid":"n1","ip":"10.0.0.9","name":"A","qport":8899}"#).unwrap();
        assert_eq!(bt_nsd_inject_device(j.as_ptr()), 0);
        let u = CString::new("n1").unwrap();
        assert_eq!(bt_nsd_remove_device(u.as_ptr()), 0);

        // 未连接设备 → -1
        let bad = CString::new("no-such-device").unwrap();
        assert_eq!(
            bt_connect(bad.as_ptr()),
            utils::BtError::InvalidArgument.code()
        );

        bt_shutdown();
        // 事件分发线程应至少推过 DEVICE_LIST（NSD 注入触发）
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(LAST_EVENT.load(Ordering::SeqCst) != 0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
