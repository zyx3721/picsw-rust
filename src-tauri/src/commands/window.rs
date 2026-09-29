use tauri::Manager;

/// 退出整个应用
#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// 隐藏主窗口到系统托盘
#[tauri::command]
pub fn hide_main_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}
