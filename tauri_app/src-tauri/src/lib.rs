//! LocalTransfer 桌面端入口（Tauri v2）。
//!
//! 核心能力全部经 lt-ffi 的 C ABI 使用（与 Android 端共用同一动态库语义）；
//! FFI 事件回调经 tauri `Emitter` 转发到前端（事件名 `lt://event`）。

mod ffi_bridge;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            ffi_bridge::init(app.handle().clone())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ffi_bridge::get_devices,
            ffi_bridge::get_tasks,
            ffi_bridge::get_config,
            ffi_bridge::set_config,
            ffi_bridge::get_local_fingerprint,
            ffi_bridge::get_local_info,
            ffi_bridge::version,
            ffi_bridge::connect,
            ffi_bridge::connect_addr,
            ffi_bridge::disconnect,
            ffi_bridge::respond_pair,
            ffi_bridge::respond_transfer,
            ffi_bridge::send_files,
            ffi_bridge::cancel_task,
            ffi_bridge::clear_records,
            ffi_bridge::clear_temp_cache,
            ffi_bridge::start_discovery,
            ffi_bridge::stop_discovery,
            ffi_bridge::probe_network,
            ffi_bridge::add_manual_device,
            ffi_bridge::inspect_paths,
            ffi_bridge::reveal_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
