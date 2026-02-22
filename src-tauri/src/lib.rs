use reqwest::Client;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

mod download;
mod filesystem;
mod instance;
mod models;

#[tauri::command]
async fn close_current_window(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_new_instance_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(existing) = app_handle.get_webview_window("new-instance") {
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let main_window = app_handle
        .get_webview_window("main")
        .ok_or("main window not found")?;

    WebviewWindowBuilder::new(
        &app_handle,
        "new-instance",
        WebviewUrl::App("/new-instance".into()),
    )
    .title("nuko | New Instance")
    .inner_size(600., 500.)
    .parent(&main_window)
    .map_err(|e| e.to_string())?
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            open_new_instance_window,
            close_current_window,
            download::get_vanilla_versions,
            download::get_paper_versions,
            download::get_fabric_game_versions,
            download::get_fabric_loader_versions,
            download::get_forge_mc_versions,
            download::get_forge_versions,
            download::get_purpur_versions,
            download::get_neoforge_mc_versions,
            download::get_neoforge_versions,
            instance::create_instance,
            instance::list_instances,
            instance::open_instance_view,
            instance::start_instance,
            instance::stop_instance,
            instance::kill_instance,
            instance::restart_instance,
            instance::get_instance_logs,
            instance::get_instance_info,
            instance::get_instance_metrics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
