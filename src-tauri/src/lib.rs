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

use std::collections::HashSet;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tokio::sync::mpsc;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub queue: mpsc::Sender<i64>,
    pub cancellations: Mutex<HashSet<i64>>,
}

impl AppState {
    pub fn request_cancel(&self, task_id: i64) {
        if let Ok(mut pending) = self.cancellations.lock() {
            pending.insert(task_id);
        }
    }

    pub fn is_task_cancelled(&self, task_id: Option<i64>) -> bool {
        let Some(task_id) = task_id else { return false };
        self.cancellations
            .lock()
            .map(|pending| pending.contains(&task_id))
            .unwrap_or(false)
    }

    pub fn clear_cancel(&self, task_id: i64) {
        if let Ok(mut pending) = self.cancellations.lock() {
            pending.remove(&task_id);
        }
    }
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

pub(crate) fn exit_app_with_interrupt(app: &tauri::AppHandle) {
    task_queue::interrupt_running_tasks(app);
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .setup(|app| {
            commands::update::cleanup_update_temp();
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("failed to resolve app data dir: {error}"))?;
            let conn = db::init_db(&data_dir).map_err(|error| format!("failed to initialize database: {error}"))?;
            let (sender, receiver) = mpsc::channel::<i64>(100);
            app.manage(AppState {
                db: Mutex::new(conn),
                queue: sender,
                cancellations: Mutex::new(HashSet::new()),
            });
            task_queue::start_workers(app.handle().clone(), receiver);

            let show_item = MenuItem::with_id(app, "show", "显示窗口 / Show Window", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出应用 / Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().expect("default window icon").clone())
                .tooltip("图床转站助手 / PicBed Switcher")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "quit" => exit_app_with_interrupt(app),
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
            commands::convert::resume_convert_task,
            commands::convert::cancel_convert_task,
            commands::convert::list_convert_tasks,
            commands::convert::get_convert_task,
            commands::convert::list_records,
            commands::convert::get_record,
            commands::convert::delete_records,
            commands::files::write_text_file,
            commands::files::scan_image_directory,
            commands::download::create_download_task,
            commands::sync::sync_save_credential,
            commands::sync::sync_load_credential,
            commands::sync::sync_delete_credential,
            commands::sync::sync_save_sync_password,
            commands::sync::sync_load_sync_password,
            commands::sync::sync_delete_sync_password,
            commands::sync::sync_load_legacy_credential,
            commands::github_sync::github_device_flow_start,
            commands::github_sync::github_device_flow_poll,
            commands::github_sync::sync_github_client_id,
            commands::update::get_update_runtime,
            commands::update::fetch::get_update_latest,
            commands::update::download::download_update,
            commands::update::install::apply_update,
            commands::update::install::open_update_package,
            commands::window::exit_app,
            commands::window::hide_main_window,
        ])
        .build(tauri::generate_context!())
        .expect("error while building picsw-rust");
    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            task_queue::interrupt_running_tasks(app_handle);
        }
    });
}
