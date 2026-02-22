use std::fs;
use std::path::PathBuf;
use tauri::Manager;

use chrono::Utc;

use crate::models::{Instance, InstanceConfig, JavaConfig, MetadataConfig};

/// Get the application's data directory, creating it if it doesn't exist
pub fn get_data_dir(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create data dir: {}", e))?;

    Ok(data_dir)
}

/// Create a new instance directory with the given name, software, version, and optional loader
/// along with downloading logic
pub async fn create_directory(data_dir: PathBuf, name: &String) -> Result<PathBuf, String> {
    let instance_dir = data_dir.join("instances").join(name);

    fs::create_dir_all(&instance_dir)
        .map_err(|e| format!("Failed to create instance directory: {}", e))?;

    Ok(instance_dir)
}

pub async fn create_eula_txt(instance_dir: &PathBuf) -> Result<(), String> {
    let eula_path = instance_dir.join("eula.txt");
    fs::write(&eula_path, "eula=true").map_err(|e| format!("Failed to create eula.txt: {}", e))?;
    Ok(())
}

pub async fn create_nuko_properties(
    instance_dir: &PathBuf,
    instance: &Instance,
) -> Result<(), String> {
    let properties_path = instance_dir.join("nuko.toml");

    let config = InstanceConfig {
        id: uuid::Uuid::new_v4().to_string(),
        custom_jar_path: instance.custom_jar_path.clone(),
        name: instance.name.clone(),
        software: instance.software.clone(),
        version: instance.version.clone(),
        loader: instance.loader.clone(),
        java: JavaConfig {
            min_memory: "2G".to_string(),
            max_memory: "4G".to_string(),
            java_path: None,
            additional_args: vec![],
        },
        metadata: MetadataConfig {
            created_at: Utc::now().to_rfc3339(),
            last_played: None,
            play_time_minutes: 0,
        },
    };

    let toml_string = toml::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize nuko.toml: {}", e))?;

    fs::write(&properties_path, toml_string)
        .map_err(|e| format!("Failed to write nuko.toml: {}", e))?;

    Ok(())
}
