use crate::{
    download::download_server_jar,
    filesystem::{self, create_eula_txt, create_nuko_properties},
    models::{Instance, InstanceInfo},
};
use tauri::Emitter;

/// Create a new Minecraft server instance with the given name, software, version, and optional loader
#[tauri::command]
pub async fn create_instance(
    app_handle: tauri::AppHandle,
    name: String,
    software: String,
    version: String,
    loader: Option<String>,
    icon_path: Option<String>,
    custom_jar_path: Option<String>,
) -> Result<(), String> {
    let server = Instance {
        name,
        software,
        version,
        loader,
        custom_jar_path,
    };

    let data_dir = filesystem::get_data_dir(&app_handle)?;

    if data_dir.join("instances").join(&server.name).exists() {
        return Err(format!("Instance '{}' already exists", server.name));
    }

    let instance_dir = filesystem::create_directory(data_dir, &server.name)
        .await
        .map_err(|e| format!("Error calling create_directory: {}", e))?;

    if let Some(icon) = icon_path {
        std::fs::copy(&icon, instance_dir.join("server-icon.png"))
            .map_err(|e| format!("Failed to copy server icon: {}", e))?;
    }

    create_nuko_properties(&instance_dir, &server)
        .await
        .map_err(|e| format!("Error calling create_nuko_manifest: {}", e))?;

    download_server_jar(&instance_dir, server)
        .await
        .map_err(|e| format!("Error calling download_server_jar: {}", e))?;

    create_eula_txt(&instance_dir)
        .await
        .map_err(|e| format!("Error calling create_eula_txt: {}", e))?;

    let _ = app_handle.emit("instances-updated", ());

    Ok(())
}

/// Lists all existing instances by reading the data directory and returning the name
/// stored in nuko.toml of subdirectories in the instances folder, and whether they're
/// running or not
#[tauri::command]
pub async fn list_instances(app_handle: tauri::AppHandle) -> Result<Vec<InstanceInfo>, String> {
    let data_dir = filesystem::get_data_dir(&app_handle)?;
    let instances_dir = data_dir.join("instances");

    if !instances_dir.exists() {
        return Ok(vec![]);
    }

    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let mut instances = Vec::new();

    for item in std::fs::read_dir(instances_dir)
        .map_err(|e| format!("Failed to read instances directory: {}", e))?
    {
        let entry = item.map_err(|e| format!("Failed to read instance entry: {}", e))?;
        if entry
            .file_type()
            .map_err(|e| format!("Failed to get file type: {}", e))?
            .is_dir()
        {
            let config_path = entry.path().join("nuko.toml");
            if config_path.exists() {
                let config_content = std::fs::read_to_string(&config_path)
                    .map_err(|e| format!("Failed to read nuko.toml: {}", e))?;
                let config: crate::models::InstanceConfig = toml::from_str(&config_content)
                    .map_err(|e| format!("Failed to parse nuko.toml: {}", e))?;

                let instance_path = entry.path();
                let mut running = false;
                for (_pid, process) in sys.processes() {
                    if let Some(cwd) = process.cwd() {
                        if cwd == instance_path {
                            running = true;
                            break;
                        }
                    }
                }

                instances.push(InstanceInfo {
                    name: config.name,
                    software: config.software,
                    version: config.version,
                    running,
                });
            }
        }
    }

    Ok(instances)
}
