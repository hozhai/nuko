use serde::{Deserialize, Serialize};

// ============ Versions ============

#[derive(Deserialize)]
pub struct MojangVersionManifest {
    pub versions: Vec<MojangVersion>,
}

#[derive(Deserialize)]
pub struct MojangVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
}

#[derive(Deserialize)]
pub struct PaperProjectResponse {
    pub versions: Vec<String>,
}

#[derive(Deserialize)]
pub struct FabricGameVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Deserialize)]
pub struct FabricLoaderVersion {
    pub loader: FabricLoader,
}

#[derive(Deserialize)]
pub struct FabricLoader {
    pub version: String,
}

// ============ Instances ============

#[derive(Debug)]
pub struct Instance {
    pub name: String,
    pub software: String,
    pub version: String,
    pub loader: Option<String>,
    pub custom_jar_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub id: String,
    pub name: String,
    pub software: String,
    pub version: String,
    pub loader: Option<String>,
    pub custom_jar_path: Option<String>,
    pub java: JavaConfig,
    pub metadata: MetadataConfig,
}

#[derive(Debug, Serialize)]
pub struct InstanceInfo {
    pub id: String,
    pub name: String,
    pub software: String,
    pub version: String,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceMetrics {
    pub time: String,
    pub cpu_usage: f32,
    pub memory_usage: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JavaConfig {
    pub min_memory: String,
    pub max_memory: String,
    pub java_path: Option<String>,
    pub additional_args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetadataConfig {
    pub created_at: String,
    pub last_played: Option<String>,
    pub play_time_minutes: u64,
}

// ============ Download (Vanilla) ============

#[derive(Deserialize)]
pub struct VersionManifest {
    pub versions: Vec<VersionEntry>,
}

#[derive(Deserialize)]
pub struct VersionEntry {
    pub id: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct VersionDetails {
    pub downloads: VersionDownloads,
}

#[derive(Deserialize)]
pub struct VersionDownloads {
    pub server: DownloadItem,
}

#[derive(Deserialize)]
pub struct DownloadItem {
    pub url: String,
    pub sha1: String,
}

// ============ Download (Paper) ============

#[derive(Deserialize)]
pub struct PaperBuilds {
    pub builds: Vec<PaperBuild>,
}

#[derive(Deserialize)]
#[serde(transparent)]
pub struct PaperBuild {
    pub build: u32,
}

#[derive(Deserialize)]
pub struct PaperDownload {
    pub downloads: PaperArtifacts,
}

#[derive(Deserialize)]
pub struct PaperArtifacts {
    pub application: PaperFile,
}

#[derive(Deserialize)]
pub struct PaperFile {
    pub name: String,
    pub sha256: String,
}

// ============ Download (Fabric) ============

#[derive(Deserialize)]
pub struct FabricLoaderResponse {
    pub launcherMeta: FabricLauncherMeta,
}

#[derive(Deserialize)]
pub struct FabricLauncherMeta {
    pub server: FabricServer,
}

#[derive(Deserialize)]
pub struct FabricServer {
    pub url: String,
}

// ============ Config ============

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub theme: String,
}
