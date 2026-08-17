use std::path::PathBuf;
use std::io::BufRead;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::Emitter;

const MODELS: &[(&str, &str, u64, &str)] = &[
    ("pipeline", "基础模型 (Pipeline)", 1_800_000_000, "https://huggingface.co/opendatalab/PDF-Extract-Kit"),
    ("vlm", "高质量模型 (VLM)", 3_200_000_000, "https://huggingface.co/opendatalab/PDF-Extract-Kit-VLM"),
];

fn model_cache_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    };
    base.join(".cache").join("mineru").join("models")
}

fn mineru_config_path() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    };
    base.join(".mineru").join("mineru.json")
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfoDto {
    pub path: String,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressDto {
    pub model_name: String,
    pub progress: f32,
    pub speed: String,
    pub stage: String,
}

pub struct ModelManager {
    downloading: Arc<AtomicBool>,
    download_cancel: Arc<AtomicBool>,
}

impl ModelManager {
    pub fn new() -> Self {
        ModelManager {
            downloading: Arc::new(AtomicBool::new(false)),
            download_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfoDto>, String> {
        let cache_dir = model_cache_dir();
        let mut models = Vec::new();

        for (name, display_name, size_bytes, _url) in MODELS {
            let model_dir = cache_dir.join(name);
            let status = if model_dir.exists() {
                let has_files = std::fs::read_dir(&model_dir)
                    .map(|entries| entries.flatten().count() > 0)
                    .unwrap_or(false);
                if has_files { "downloaded" } else { "not_downloaded" }
            } else {
                "not_downloaded"
            };

            let path = if status == "downloaded" {
                Some(model_dir.to_string_lossy().to_string())
            } else {
                None
            };

            models.push(ModelInfoDto {
                name: name.to_string(),
                display_name: display_name.to_string(),
                size_bytes: *size_bytes,
                status: status.to_string(),
                path,
                download_url: None,
                version: None,
            });
        }

        Ok(models)
    }

    pub async fn get_model_status(&self, model_name: &str) -> Result<ModelInfoDto, String> {
        let models = self.list_models().await?;
        models
            .into_iter()
            .find(|m| m.name == model_name)
            .ok_or_else(|| format!("模型 '{}' 不存在", model_name))
    }

    pub async fn download_model(&self, app: &tauri::AppHandle, model_name: &str) -> Result<(), String> {
        if self.downloading.swap(true, Ordering::Relaxed) {
            return Err("已有下载任务在进行中".to_string());
        }
        self.download_cancel.store(false, Ordering::Relaxed);

        let cancel_flag = self.download_cancel.clone();
        let app_clone = app.clone();
        let name = model_name.to_string();

        let result = tokio::task::spawn_blocking(move || {
            let mut cmd = std::process::Command::new("mineru-models-download");
            cmd.arg("--model").arg(&name);
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            let mut child = cmd.spawn().map_err(|e| {
                format!("无法启动 mineru-models-download: {}", e)
            })?;

            let stdout = child.stdout.take().unwrap();
            let reader = std::io::BufReader::new(stdout);

            for line in reader.lines() {
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = child.kill();
                    break;
                }

                if let Ok(l) = line {
                    let l = l.trim().to_string();
                    if l.starts_with("Downloading:") || l.starts_with("下载中:") {
                        let pct = l
                            .split(|c: char| c == ':' || c == '%')
                            .nth(1)
                            .and_then(|s| s.trim().parse::<f32>().ok())
                            .unwrap_or(0.0);
                        let _ = app_clone.emit(
                            "model-download-progress",
                            DownloadProgressDto {
                                model_name: name.clone(),
                                progress: pct / 100.0,
                                speed: "".to_string(),
                                stage: "downloading".to_string(),
                            },
                        );
                    }
                }
            }

            let status = child.wait().map_err(|e| format!("等待 mineru-models-download 失败: {}", e))?;
            if !status.success() {
                return Err("模型下载失败".to_string());
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("下载任务异常: {}", e))?;

        self.downloading.store(false, Ordering::Relaxed);
        result
    }

    pub async fn cancel_download(&self, app: &tauri::AppHandle) -> Result<(), String> {
        self.download_cancel.store(true, Ordering::Relaxed);
        let _ = app.emit(
            "model-download-progress",
            DownloadProgressDto {
                model_name: "".to_string(),
                progress: 0.0,
                speed: "".to_string(),
                stage: "cancelled".to_string(),
            },
        );
        Ok(())
    }

    pub async fn get_cache_info(&self) -> Result<CacheInfoDto, String> {
        let cache_dir = model_cache_dir();
        let total_size = dir_size(&cache_dir);
        Ok(CacheInfoDto {
            path: cache_dir.to_string_lossy().to_string(),
            total_size_bytes: total_size,
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

    pub async fn set_source(&self, source: String) -> Result<(), String> {
        let config_path = mineru_config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
        }

        let config = if config_path.exists() {
            std::fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        let mut parsed: serde_json::Value =
            serde_json::from_str(&config).unwrap_or(serde_json::json!({}));
        parsed["model_source"] = serde_json::json!(source);

        let content = serde_json::to_string_pretty(&parsed)
            .map_err(|e| format!("序列化配置失败: {}", e))?;
        std::fs::write(&config_path, &content)
            .map_err(|e| format!("写入配置失败: {}", e))?;

        Ok(())
    }

    pub async fn get_source(&self) -> Result<String, String> {
        let config_path = mineru_config_path();
        if !config_path.exists() {
            return Ok("auto".to_string());
        }
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("读取配置失败: {}", e))?;
        let parsed: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| format!("解析配置失败: {}", e))?;
        Ok(parsed["model_source"]
            .as_str()
            .unwrap_or("auto")
            .to_string())
    }

    pub async fn import_offline(&self, _app: &tauri::AppHandle, path: &str) -> Result<(), String> {
        let cache_dir = model_cache_dir();
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("创建缓存目录失败: {}", e))?;

        let src = std::path::Path::new(path);
        if !src.exists() {
            return Err(format!("离线包路径不存在: {}", path));
        }

        let dest = cache_dir.join(
            src.file_name()
                .ok_or_else(|| "无效的文件名".to_string())?,
        );

        if src.is_dir() {
            copy_dir_recursive(src, &dest)?;
        } else {
            std::fs::copy(src, &dest).map_err(|e| format!("复制文件失败: {}", e))?;
        }

        Ok(())
    }

    pub async fn check_update(&self, _model_name: &str) -> Result<bool, String> {
        Ok(false)
    }
}

fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                total += dir_size(&path);
            } else if let Ok(meta) = path.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("创建目录失败: {}", e))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("读取目录失败: {}", e))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let file_type = entry.file_type().map_err(|e| format!("获取文件类型失败: {}", e))?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path)
                .map_err(|e| format!("复制文件 {} 失败: {}", src_path.display(), e))?;
        }
    }
    Ok(())
}