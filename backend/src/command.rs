//! Tauri Command Handlers
//!
//! This module contains all Tauri commands that can be invoked from the frontend.

use tauri::AppHandle;
use vrm2sl_tauri_lib::{
    LogLevel,
    convert::{AnalysisReport, ConversionReport},
    ipc::{
        AnalyzeRequest, ConvertRequest, LoadSettingsRequest, SaveSettingsRequest, analyze_vrm_ipc,
        convert_vrm_to_gdb_ipc, load_project_settings_ipc, save_project_settings_ipc,
    },
    project::ProjectSettings,
    send_log_with_handle,
};

#[tauri::command]
pub async fn analyze_vrm_command(
    request: AnalyzeRequest,
    app: AppHandle,
) -> Result<AnalysisReport, String> {
    send_log_with_handle(
        &app,
        LogLevel::Info,
        &format!("Analyze VRM request: {}", request.input_path),
    );
    let result = analyze_vrm_ipc(request);
    if result.is_ok() {
        send_log_with_handle(&app, LogLevel::Info, "Analyze VRM completed");
    }
    result
}

/// Get the application version
///
/// # Returns
/// * `Ok(String)` - The application version
#[tauri::command]
pub async fn get_app_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[tauri::command]
pub async fn convert_vrm_command(
    request: ConvertRequest,
    app: AppHandle,
) -> Result<ConversionReport, String> {
    send_log_with_handle(
        &app,
        LogLevel::Info,
        &format!(
            "Convert VRM request: {} -> {}",
            request.input_path, request.output_path
        ),
    );
    let result = convert_vrm_to_gdb_ipc(request);
    if result.is_ok() {
        send_log_with_handle(&app, LogLevel::Info, "Convert VRM completed");
    }
    result
}

#[tauri::command]
pub async fn save_project_settings_command(
    request: SaveSettingsRequest,
    app: AppHandle,
) -> Result<(), String> {
    send_log_with_handle(&app, LogLevel::Info, "Save project settings request");
    save_project_settings_ipc(request)
}

#[tauri::command]
pub async fn load_project_settings_command(
    request: LoadSettingsRequest,
    app: AppHandle,
) -> Result<ProjectSettings, String> {
    send_log_with_handle(&app, LogLevel::Info, "Load project settings request");
    load_project_settings_ipc(request)
}
