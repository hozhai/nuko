use std::{fs, path::Path};

use reqwest::Client;

use crate::models::{
    self, FabricLoaderResponse, Instance, PaperBuilds, PaperDownload, VersionDetails,
    VersionManifest,
};

/// Download the appropriate server JAR for the given instance
pub async fn download_server_jar(instance_dir: &Path, instance: Instance) -> Result<(), String> {
    println!(
        "Resolving download URL for {} {}...",
        instance.software, instance.version
    );
    let url = match instance.software.as_str() {
        "vanilla" => resolve_vanilla_url(&instance.version).await?,
        "papermc" => resolve_paper_url(&instance.version).await?,
        "purpur" => resolve_purpur_url(&instance.version).await?,
        "fabric" => resolve_fabric_url(&instance.version, instance.loader.as_deref()).await?,
        "forge" => {
            let loader = instance
                .loader
                .as_deref()
                .ok_or_else(|| "Forge requires a loader/installer version".to_string())?;
            println!("Installing Forge {}...", loader);
            return install_forge(instance_dir, &instance.version, loader).await;
        }
        "neoforge" => {
            let loader = instance
                .loader
                .as_deref()
                .ok_or_else(|| "NeoForge requires a loader/installer version".to_string())?;
            println!("Installing NeoForge {}...", loader);
            return install_neoforge(instance_dir, &instance.version, loader).await;
        }
        "custom" => {
            let custom_path = instance
                .custom_jar_path
                .as_deref()
                .ok_or_else(|| "Custom software requires a custom_jar_path".to_string())?;
            let jar_path = instance_dir.join("server.jar");
            println!(
                "Copying custom jar from {} to {}...",
                custom_path,
                jar_path.display()
            );
            fs::copy(custom_path, &jar_path)
                .map_err(|e| format!("Failed to copy custom jar: {}", e))?;
            println!("Copy complete!");
            return Ok(());
        }
        other => return Err(format!("Unsupported software '{}'", other)),
    };

    let jar_path = instance_dir.join("server.jar");
    println!(
        "Downloading server jar from {} to {}...",
        url,
        jar_path.display()
    );
    download_to_path(&url, &jar_path).await?;
    println!("Download complete!");
    Ok(())
}

async fn download_to_path(url: &str, path: &Path) -> Result<(), String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("GET {} failed: {}", url, e))?;
    if !response.status().is_success() {
        return Err(format!("{} -> HTTP {}", url, response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Reading body failed: {}", e))?;

    fs::write(path, &bytes).map_err(|e| format!("Writing {} failed: {}", path.display(), e))?;
    Ok(())
}

async fn resolve_vanilla_url(version: &str) -> Result<String, String> {
    const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

    let manifest: VersionManifest = reqwest::get(MANIFEST)
        .await
        .map_err(|e| format!("Failed to fetch version manifest: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse version manifest: {}", e))?;

    let entry = manifest
        .versions
        .into_iter()
        .find(|v| v.id == version)
        .ok_or_else(|| format!("Version {} not found in Mojang manifest", version))?;

    let details: VersionDetails = reqwest::get(entry.url)
        .await
        .map_err(|e| format!("fetch version details failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("parse version details failed: {}", e))?;

    Ok(details.downloads.server.url)
}

async fn resolve_paper_url(version: &str) -> Result<String, String> {
    let builds_url = format!(
        "https://api.papermc.io/v2/projects/paper/versions/{}",
        version
    );
    let builds: PaperBuilds = reqwest::get(&builds_url)
        .await
        .map_err(|e| format!("fetch Paper builds failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("parse Paper builds failed: {}", e))?;

    let latest = builds
        .builds
        .last()
        .ok_or_else(|| format!("No Paper builds for {}", version))?
        .build;

    let meta_url = format!(
        "https://api.papermc.io/v2/projects/paper/versions/{}/builds/{}",
        version, latest
    );
    let meta: PaperDownload = reqwest::get(&meta_url)
        .await
        .map_err(|e| format!("fetch Paper build meta failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("parse Paper build meta failed: {}", e))?;

    let download = format!(
        "https://api.papermc.io/v2/projects/paper/versions/{}/builds/{}/downloads/{}",
        version, latest, meta.downloads.application.name
    );

    Ok(download)
}

async fn resolve_fabric_url(
    mc_version: &str,
    loader_version: Option<&str>,
) -> Result<String, String> {
    let loader = loader_version.ok_or_else(|| "Fabric loader version missing".to_string())?;

    #[derive(serde::Deserialize)]
    struct Installer {
        version: String,
    }

    let installers: Vec<Installer> =
        reqwest::get("https://meta.fabricmc.net/v2/versions/installer")
            .await
            .map_err(|e| format!("fetch Fabric installer versions failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("parse Fabric installer versions failed: {}", e))?;

    let installer_version = &installers
        .first()
        .ok_or_else(|| "No Fabric installer versions found".to_string())?
        .version;

    Ok(format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar",
        mc_version, loader, installer_version
    ))
}

async fn install_forge(
    instance_dir: &Path,
    mc_version: &str,
    forge_version: &str,
) -> Result<(), String> {
    let artifact = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{mv}-{fv}/forge-{mv}-{fv}-installer.jar",
        mv = mc_version,
        fv = forge_version
    );
    let installer_path = instance_dir.join("forge-installer.jar");
    download_to_path(&artifact, &installer_path).await?;

    let status = std::process::Command::new("java")
        .current_dir(instance_dir)
        .arg("-jar")
        .arg(&installer_path)
        .arg("--installServer")
        .status()
        .map_err(|e| format!("Starting Forge installer failed: {}", e))?;

    if !status.success() {
        return Err(format!("Forge installer exited with {}", status));
    }

    let _ = fs::remove_file(&installer_path);
    let _ = fs::remove_file(instance_dir.join("forge-installer.jar.log"));

    if let Ok(entries) = fs::read_dir(instance_dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if name.starts_with("forge-") && name.ends_with(".jar") && name != "forge-installer.jar"
            {
                let _ = fs::rename(entry.path(), instance_dir.join("server.jar"));
                break;
            }
        }
    }

    Ok(())
}

async fn resolve_purpur_url(version: &str) -> Result<String, String> {
    Ok(format!(
        "https://api.purpurmc.org/v2/purpur/{}/latest/download",
        version
    ))
}

async fn install_neoforge(
    instance_dir: &Path,
    _mc_version: &str,
    neoforge_version: &str,
) -> Result<(), String> {
    let artifact = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{fv}/neoforge-{fv}-installer.jar",
        fv = neoforge_version
    );
    let installer_path = instance_dir.join("neoforge-installer.jar");
    download_to_path(&artifact, &installer_path).await?;

    let status = std::process::Command::new("java")
        .current_dir(instance_dir)
        .arg("-jar")
        .arg(&installer_path)
        .arg("--installServer")
        .status()
        .map_err(|e| format!("Starting NeoForge installer failed: {}", e))?;

    if !status.success() {
        return Err(format!("NeoForge installer exited with {}", status));
    }

    let _ = fs::remove_file(&installer_path);
    let _ = fs::remove_file(instance_dir.join("neoforge-installer.jar.log"));

    if let Ok(entries) = fs::read_dir(instance_dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if name.starts_with("neoforge-")
                && name.ends_with(".jar")
                && name != "neoforge-installer.jar"
            {
                let _ = fs::rename(entry.path(), instance_dir.join("server.jar"));
                break;
            }
        }
    }

    Ok(())
}

/// Fetch Vanilla Minecraft versions from Mojang API
/// Returns only release versions, sorted newest first
#[tauri::command]
pub async fn get_vanilla_versions() -> Result<Vec<String>, String> {
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
pub async fn get_paper_versions() -> Result<Vec<String>, String> {
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
pub async fn get_fabric_game_versions() -> Result<Vec<String>, String> {
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
pub async fn get_fabric_loader_versions(mc_version: String) -> Result<Vec<String>, String> {
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
pub async fn get_forge_mc_versions() -> Result<Vec<String>, String> {
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
pub async fn get_forge_versions(mc_version: String) -> Result<Vec<String>, String> {
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
pub async fn get_purpur_versions() -> Result<Vec<String>, String> {
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
pub async fn get_neoforge_mc_versions() -> Result<Vec<String>, String> {
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
pub async fn get_neoforge_versions(mc_version: String) -> Result<Vec<String>, String> {
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
