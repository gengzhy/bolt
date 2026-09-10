//! LocalTransfer 桌面端入口（Tauri v2）。
//!
//! 核心能力全部经 lt-ffi 的 C ABI 使用（与 Android 端共用同一动态库语义）；
//! FFI 事件回调经 tauri `Emitter` 转发到前端（事件名 `lt://event`）。

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

mod ffi_bridge;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            ffi_bridge::init(app.handle().clone())?;

            let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let mut tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                });

            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            let _ = tray_builder.build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let cfg = ffi_bridge::get_config();
                let minimize_to_tray = cfg
                    .get("minimize_to_tray")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if minimize_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
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
