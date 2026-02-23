use std::fs;
use tauri::{AppHandle, Emitter};

use crate::filesystem::get_data_dir;
use crate::models::GlobalConfig;

#[tauri::command]
pub fn get_config(app_handle: AppHandle) -> Result<GlobalConfig, String> {
    let data_dir = get_data_dir(&app_handle)?;
    let config_path = data_dir.join("config.toml");

    if !config_path.exists() {
        let default_config = GlobalConfig {
            theme: "dark".to_string(),
        };
        let toml_string = toml::to_string_pretty(&default_config)
            .map_err(|e| format!("Failed to serialize default config: {}", e))?;
        fs::write(&config_path, toml_string)
            .map_err(|e| format!("Failed to write default config: {}", e))?;
        return Ok(default_config);
    }

    let config_str = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.toml: {}", e))?;

    let config: GlobalConfig =
        toml::from_str(&config_str).map_err(|e| format!("Failed to parse config.toml: {}", e))?;

    Ok(config)
}

#[tauri::command]
pub fn set_theme(app_handle: AppHandle, theme: String) -> Result<(), String> {
    let data_dir = get_data_dir(&app_handle)?;
    let config_path = data_dir.join("config.toml");

    let mut config = if config_path.exists() {
        let config_str = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config.toml: {}", e))?;
        toml::from_str(&config_str).unwrap_or_else(|_| GlobalConfig {
            theme: theme.clone(),
        })
    } else {
        GlobalConfig {
            theme: theme.clone(),
        }
    };

    config.theme = theme.clone();

    let toml_string = toml::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&config_path, toml_string)
        .map_err(|e| format!("Failed to write config.toml: {}", e))?;

    app_handle
        .emit("theme-changed", theme)
        .map_err(|e| format!("Failed to emit theme-changed event: {}", e))?;

    Ok(())
}
