use std::{fs, path::Path};

use crate::models::{
    FabricLoaderResponse, Instance, PaperBuilds, PaperDownload, VersionDetails, VersionManifest,
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
