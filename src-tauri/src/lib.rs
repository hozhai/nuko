use reqwest::Client;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

mod download;
mod filesystem;
mod instance;
mod models;

/// Fetch Vanilla Minecraft versions from Mojang API
/// Returns only release versions, sorted newest first
#[tauri::command]
async fn get_vanilla_versions() -> Result<Vec<String>, String> {
    let client = Client::new();
    let response = client
        .get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Mojang versions: {}", e))?;

    let manifest: models::MojangVersionManifest = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Mojang response: {}", e))?;

    let versions: Vec<String> = manifest
        .versions
        .into_iter()
        .filter(|v| v.version_type == "release")
        .map(|v| v.id)
        .collect();

    Ok(versions)
}

/// Fetch PaperMC supported Minecraft versions
/// Returns versions sorted newest first
#[tauri::command]
async fn get_paper_versions() -> Result<Vec<String>, String> {
    let client = Client::new();
    let response = client
        .get("https://api.papermc.io/v2/projects/paper")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Paper versions: {}", e))?;

    let project: models::PaperProjectResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Paper response: {}", e))?;

    // Paper API returns versions oldest-first, so reverse them
    let mut versions = project.versions;
    versions.reverse();

    Ok(versions)
}

/// Fetch Fabric-supported Minecraft versions
/// Returns only stable versions, sorted newest first
#[tauri::command]
async fn get_fabric_game_versions() -> Result<Vec<String>, String> {
    let client = Client::new();
    let response = client
        .get("https://meta.fabricmc.net/v2/versions/game")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Fabric game versions: {}", e))?;

    let versions: Vec<models::FabricGameVersion> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Fabric game versions: {}", e))?;

    // Filter to stable versions only (already sorted newest first by the API)
    let versions: Vec<String> = versions
        .into_iter()
        .filter(|v| v.stable)
        .map(|v| v.version)
        .collect();

    Ok(versions)
}

/// Fetch Fabric loader versions compatible with a specific Minecraft version
/// Returns loader versions sorted newest first
#[tauri::command]
async fn get_fabric_loader_versions(mc_version: String) -> Result<Vec<String>, String> {
    let client = Client::new();
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}",
        mc_version
    );

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Fabric loader versions: {}", e))?;

    let loaders: Vec<models::FabricLoaderVersion> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Fabric loader versions: {}", e))?;

    // Return all loader versions (already sorted newest first by the API)
    let versions: Vec<String> = loaders.into_iter().map(|l| l.loader.version).collect();

    Ok(versions)
}

/// Fetch Minecraft versions that have Forge support
/// Returns versions sorted newest first
#[tauri::command]
async fn get_forge_mc_versions() -> Result<Vec<String>, String> {
    let client = Client::new();
    let response = client
        .get("https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Forge versions: {}", e))?;

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Forge versions: {}", e))?;

    // Extract unique MC versions from version tags like <version>1.20.1-47.2.0</version>
    let mut mc_versions: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if let Some(inner) = trimmed
                .strip_prefix("<version>")
                .and_then(|s| s.strip_suffix("</version>"))
            {
                // MC version is the part before the first dash
                inner.split('-').next().map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();

    // Remove duplicates
    mc_versions.sort();
    mc_versions.dedup();

    // Sort by version number (newest first)
    mc_versions.sort_by(|a, b| {
        let a_parts: Vec<u32> = a.split('.').filter_map(|p| p.parse().ok()).collect();
        let b_parts: Vec<u32> = b.split('.').filter_map(|p| p.parse().ok()).collect();
        b_parts.cmp(&a_parts)
    });

    Ok(mc_versions)
}

/// Fetch Forge versions for a specific Minecraft version from Maven metadata
/// Returns all available versions, sorted newest first
#[tauri::command]
async fn get_forge_versions(mc_version: String) -> Result<Vec<String>, String> {
    let client = Client::new();

    // Fetch all versions from Maven metadata
    let response = client
        .get("https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Forge versions: {}", e))?;

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Forge versions: {}", e))?;

    let prefix = format!("{}-", mc_version);

    // Parse version tags from XML and filter by MC version
    let mut versions: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if let Some(inner) = trimmed
                .strip_prefix("<version>")
                .and_then(|s| s.strip_suffix("</version>"))
            {
                if inner.starts_with(&prefix) {
                    // Extract just the Forge version part (after "mcVersion-")
                    Some(inner[prefix.len()..].to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // Sort newest first by version number
    versions.sort_by(|a, b| {
        let a_parts: Vec<u32> = a.split('.').filter_map(|p| p.parse().ok()).collect();
        let b_parts: Vec<u32> = b.split('.').filter_map(|p| p.parse().ok()).collect();
        b_parts.cmp(&a_parts)
    });

    Ok(versions)
}

/// Fetch Purpur supported Minecraft versions
/// Returns versions sorted newest first
#[tauri::command]
async fn get_purpur_versions() -> Result<Vec<String>, String> {
    let client = Client::new();
    let response = client
        .get("https://api.purpurmc.org/v2/purpur")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Purpur versions: {}", e))?;

    #[derive(serde::Deserialize)]
    struct PurpurResponse {
        versions: Vec<String>,
    }

    let project: PurpurResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Purpur response: {}", e))?;

    let mut versions = project.versions;
    versions.reverse();

    Ok(versions)
}

/// Fetch Minecraft versions that have NeoForge support
/// Returns versions sorted newest first
#[tauri::command]
async fn get_neoforge_mc_versions() -> Result<Vec<String>, String> {
    let client = Client::new();
    let response = client
        .get("https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch NeoForge versions: {}", e))?;

    #[derive(serde::Deserialize)]
    struct NeoForgeResponse {
        versions: Vec<String>,
    }

    let project: NeoForgeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse NeoForge response: {}", e))?;

    let mut mc_versions: Vec<String> = project
        .versions
        .into_iter()
        .filter_map(|v| {
            let parts: Vec<&str> = v.split('.').collect();
            if parts.len() >= 2 {
                let major = parts[0];
                let minor = parts[1];
                if let Ok(major_num) = major.parse::<u32>() {
                    if minor == "0" {
                        Some(format!("1.{}", major_num))
                    } else {
                        Some(format!("1.{}.{}", major_num, minor))
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    mc_versions.sort();
    mc_versions.dedup();

    mc_versions.sort_by(|a, b| {
        let a_parts: Vec<u32> = a.split('.').filter_map(|p| p.parse().ok()).collect();
        let b_parts: Vec<u32> = b.split('.').filter_map(|p| p.parse().ok()).collect();
        b_parts.cmp(&a_parts)
    });

    Ok(mc_versions)
}

/// Fetch NeoForge versions for a specific Minecraft version
/// Returns versions sorted newest first
#[tauri::command]
async fn get_neoforge_versions(mc_version: String) -> Result<Vec<String>, String> {
    let client = Client::new();
    let response = client
        .get("https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch NeoForge versions: {}", e))?;

    #[derive(serde::Deserialize)]
    struct NeoForgeResponse {
        versions: Vec<String>,
    }

    let project: NeoForgeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse NeoForge response: {}", e))?;

    let prefix = if let Some(stripped) = mc_version.strip_prefix("1.") {
        let parts: Vec<&str> = stripped.split('.').collect();
        if parts.len() == 1 {
            format!("{}.0.", parts[0])
        } else if parts.len() == 2 {
            format!("{}.{}.", parts[0], parts[1])
        } else {
            return Ok(vec![]);
        }
    } else {
        return Ok(vec![]);
    };

    let mut versions: Vec<String> = project
        .versions
        .into_iter()
        .filter(|v| v.starts_with(&prefix))
        .collect();

    versions.sort_by(|a, b| {
        let a_clean = a.split('-').next().unwrap_or(a);
        let b_clean = b.split('-').next().unwrap_or(b);
        let a_parts: Vec<u32> = a_clean.split('.').filter_map(|p| p.parse().ok()).collect();
        let b_parts: Vec<u32> = b_clean.split('.').filter_map(|p| p.parse().ok()).collect();
        b_parts.cmp(&a_parts)
    });

    Ok(versions)
}

#[tauri::command]
async fn close_current_window(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_new_instance_window(app: AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("new-instance") {
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let main_window = app
        .get_webview_window("main")
        .ok_or("main window not found")?;

    WebviewWindowBuilder::new(
        &app,
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
            get_vanilla_versions,
            get_paper_versions,
            get_fabric_game_versions,
            get_fabric_loader_versions,
            get_forge_mc_versions,
            get_forge_versions,
            get_purpur_versions,
            get_neoforge_mc_versions,
            get_neoforge_versions,
            instance::create_instance,
            instance::list_instances
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
