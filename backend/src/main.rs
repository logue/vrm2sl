// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod command;

fn main() {
    tauri::Builder::default()
        // Initialize Tauri plugins
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        // Register command handlers
        .invoke_handler(tauri::generate_handler![
            command::analyze_vrm_command,
            command::convert_vrm_command,
            command::build_preview_glb_command,
            command::save_project_settings_command,
            command::load_project_settings_command,
            command::get_app_version,
        ])
        .setup(|app| {
            // Initialize logging system with app handle
            vrm2sl_tauri_lib::init_logging(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
