use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Resolve the application install directory.
///
/// In a normal Tauri bundle (nsis/msi/portable), `current_exe` points at the
/// `.exe` inside the install folder, so its parent is the install root.
fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn model_cache_dir() -> PathBuf {
    install_dir().join("models")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareRequirements {
    pub min_ram_gb: u64,
    pub rec_ram_gb: u64,
    pub gpu_required: bool,
    pub gpu_vram_gb: u64,
    pub cpu_only_supported: bool,
    pub notes: String,
}

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
    pub hardware_requirements: HardwareRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfoDto {
    pub path: String,
    pub total_size_bytes: u64,
}

/// Model management was tied to the bundled recognition engine, which has been
/// removed. This is a no-op stub that keeps the same command surface so the
/// frontend still compiles and runs; it reports no models and refuses to
/// download anything.
pub struct ModelManager;

impl ModelManager {
    pub fn new() -> Self {
        ModelManager
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfoDto>, String> {
        Ok(Vec::new())
    }

    pub async fn get_model_status(&self, model_name: &str) -> Result<ModelInfoDto, String> {
        Err(format!("模型管理已移除，未找到模型 '{}'", model_name))
    }

    pub async fn download_model(&self, _app: &tauri::AppHandle, model_name: &str) -> Result<(), String> {
        Err(format!("识别引擎已移除，无法下载模型 '{}'", model_name))
    }

    pub async fn cancel_download(&self, _app: &tauri::AppHandle) -> Result<(), String> {
        Ok(())
    }

    pub async fn get_cache_info(&self) -> Result<CacheInfoDto, String> {
        Ok(CacheInfoDto {
            path: model_cache_dir().to_string_lossy().to_string(),
            total_size_bytes: 0,
        })
    }

    pub async fn clear_cache(&self) -> Result<(), String> {
        let cache_dir = model_cache_dir();
        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir)
                .map_err(|e| format!("清理缓存失败: {}", e))?;
        }
        Ok(())
    }

    pub async fn set_source(&self, _source: String) -> Result<(), String> {
        Ok(())
    }

    pub async fn get_source(&self) -> Result<String, String> {
        Ok("auto".to_string())
    }

    pub async fn import_offline(&self, _app: &tauri::AppHandle, _path: &str) -> Result<(), String> {
        Err("识别引擎已移除，无法导入离线模型".to_string())
    }

    pub async fn check_update(&self, _model_name: &str) -> Result<bool, String> {
        Ok(false)
    }

    /// Check whether the bundled Python runtime is ready. With the recognition
    /// engine removed there is no Python runtime to check.
    pub fn check_python_environment() -> Result<bool, String> {
        Ok(false)
    }

    /// No-op: the portable Python + recognition engine runtime is no longer
    /// bundled or downloaded.
    pub async fn setup_python_environment(_app: &tauri::AppHandle) -> Result<(), String> {
        Ok(())
    }
}
