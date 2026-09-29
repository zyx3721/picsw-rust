mod commands;
mod crypto;
mod db;
mod secret_store;
mod engine;
mod markdown;
mod models;
mod task_queue;
mod types_def;
mod uploader;

use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tokio::sync::mpsc;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub queue: mpsc::Sender<i64>,
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn show_close_confirm(window: &tauri::WebviewWindow) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    let _ = window.emit("close-requested", ());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("failed to resolve app data dir: {error}"))?;
            let conn = db::init_db(&data_dir).map_err(|error| format!("failed to initialize database: {error}"))?;
            let (sender, receiver) = mpsc::channel::<i64>(100);
            app.manage(AppState { db: Mutex::new(conn), queue: sender });
            task_queue::start_workers(app.handle().clone(), receiver);

            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出应用", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().expect("default window icon").clone())
                .tooltip("图床转站助手")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let window_for_close = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        show_close_confirm(&window_for_close);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::configs::get_picbed_types,
            commands::configs::list_configs,
            commands::configs::create_config,
            commands::configs::update_config,
            commands::configs::delete_config,
            commands::configs::set_default_config,
            commands::configs::test_config_draft,
            commands::configs::test_config_saved,
            commands::convert::analyze_markdown,
            commands::convert::create_convert_task,
            commands::convert::create_local_convert_task,
            commands::convert::list_convert_tasks,
            commands::convert::get_convert_task,
            commands::convert::list_records,
            commands::convert::get_record,
            commands::convert::delete_records,
            commands::files::write_text_file,
            commands::download::download_remote_images,
            commands::sync::sync_save_credential,
            commands::sync::sync_load_credential,
            commands::sync::sync_delete_credential,
            commands::sync::sync_load_legacy_credential,
            commands::github_sync::github_device_flow_start,
            commands::github_sync::github_device_flow_poll,
            commands::github_sync::sync_github_client_id,
            commands::window::exit_app,
            commands::window::hide_main_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running picsw-rust");
}
