use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfoDto {
    pub name: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub status: String,
    pub path: Option<String>,
    pub download_url: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelStatus {
    NotDownloaded,
    Downloading,
    Downloaded,
    Error,
    Updating,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub model_name: String,
    pub progress: f32,
    pub speed: String,
    pub stage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfoDto {
    pub path: String,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfoDto {
    pub model_name: String,
    pub has_update: bool,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
}