use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// Hugging Face Hub API — model info response (only the fields we need).
#[derive(Deserialize)]
struct HfModelInfo {
    siblings: Vec<HfFile>,
}

#[derive(Deserialize)]
struct HfFile {
    rfilename: String,
    #[serde(default)]
    size: Option<u64>,
}

// ── Model metadata: (name, display_name, size_bytes, download_url,
//                      min_ram_gb, rec_ram_gb, gpu_required, gpu_vram_gb,
//                      cpu_only_supported, notes) ──
const MODELS: &[(&str, &str, u64, &str, u64, u64, bool, u64, bool, &str)] = &[
    (
        "pipeline",
        "基础模型 (Pipeline)",
        1_800_000_000,
        "https://huggingface.co/opendatalab/PDF-Extract-Kit",
        16, 16, false, 4, true,
        "仅 CPU 可运行，GPU 可加速",
    ),
    (
        "vlm",
        "高质量模型 (VLM)",
        3_200_000_000,
        "https://huggingface.co/opendatalab/PDF-Extract-Kit-VLM",
        16, 32, true, 8, false,
        "需要 Volta 及以上架构显卡",
    ),
];

/// Resolve the application install directory.
///
/// In a normal Tauri bundle (nsis/msi/portable), `current_exe` points at the
/// `.exe` inside the install folder, so its parent is the install root.
pub fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn model_cache_dir() -> PathBuf {
    install_dir().join("models")
}

fn mineru_config_path() -> PathBuf {
    install_dir().join("config").join("mineru.json")
}

fn python_dir() -> PathBuf {
    install_dir().join("python")
}

pub fn bundled_python_exe() -> PathBuf {
    python_dir().join("python.exe")
}

pub fn bundled_mineru_api() -> Option<(PathBuf, &'static str)> {
    let py = bundled_python_exe();
    if py.exists() {
        Some((py, "mineru_api"))
    } else {
        None
    }
}

pub fn is_bundled_python_ready() -> bool {
    bundled_python_exe().exists()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareRequirements {
    /// Minimum RAM in GiB.
    pub min_ram_gb: u64,
    /// Recommended RAM in GiB.
    pub rec_ram_gb: u64,
    /// Whether a dedicated GPU is strictly required.
    pub gpu_required: bool,
    /// GPU VRAM in GiB (0 if not applicable).
    pub gpu_vram_gb: u64,
    /// Whether the model can run on CPU only (without a GPU).
    pub cpu_only_supported: bool,
    /// Human-readable extra note (e.g. "Volta architecture required").
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressDto {
    pub model_name: String,
    pub progress: f32,
    pub speed: String,
    pub stage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonSetupProgressDto {
    pub stage: String,
    pub progress: f32,
    pub detail: String,
}

/// Format bytes/sec into a human-readable speed string.
fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec >= 1_000_000_000 {
        format!("{:.1} GB/s", bytes_per_sec as f64 / 1_000_000_000.0)
    } else if bytes_per_sec >= 1_000 {
        format!("{:.1} MB/s", bytes_per_sec as f64 / 1_000_000.0)
    } else {
        format!("{} B/s", bytes_per_sec)
    }
}

pub struct ModelManager {
    downloading: Arc<AtomicBool>,
    download_cancel: Arc<AtomicBool>,
}

struct DownloadingGuard(Arc<AtomicBool>);

impl Drop for DownloadingGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
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

        for tuple in MODELS {
            let (name, display_name, size_bytes, _url, min_ram, rec_ram,
                 gpu_required, gpu_vram, cpu_only, notes) = tuple;

            let model_dir = cache_dir.join(*name);
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
                hardware_requirements: HardwareRequirements {
                    min_ram_gb: *min_ram,
                    rec_ram_gb: *rec_ram,
                    gpu_required: *gpu_required,
                    gpu_vram_gb: *gpu_vram,
                    cpu_only_supported: *cpu_only,
                    notes: notes.to_string(),
                },
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
        let _downloading_guard = DownloadingGuard(self.downloading.clone());
        self.download_cancel.store(false, Ordering::Relaxed);

        let repo_id = match model_name {
            "pipeline" => "opendatalab/PDF-Extract-Kit",
            "vlm" => "opendatalab/PDF-Extract-Kit-VLM",
            _ => return Err(format!("未知模型: {}", model_name)),
        };

        let cache_dir = model_cache_dir().join(model_name);
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("创建模型目录失败: {}", e))?;

        // 1. Fetch file list from Hugging Face Hub API.
        let api_url = format!("https://huggingface.co/api/models/{}", repo_id);
        let client = reqwest::Client::builder()
            .user_agent("OmniMD/0.1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        let info_resp = client.get(&api_url).send().await.map_err(|e| {
            format!("无法连接 Hugging Face Hub ({}): {}", api_url, e)
        })?;
        if !info_resp.status().is_success() {
            return Err(format!(
                "Hugging Face Hub 返回错误 ({}): {}",
                info_resp.status(),
                info_resp.text().await.unwrap_or_default()
            ));
        }
        let info: HfModelInfo = info_resp.json().await.map_err(|e| {
            format!("解析模型文件列表失败: {}", e)
        })?;

        if info.siblings.is_empty() {
            return Err("模型仓库中没有文件".to_string());
        }

        // 2. Download each file sequentially.
        let total_size: u64 = info.siblings.iter().filter_map(|f| f.size).sum();
        let mut downloaded_bytes: u64 = 0;
        let overall_start = Instant::now();

        for (i, file) in info.siblings.iter().enumerate() {
            // Skip directories (they have empty rfilename or end with /).
            if file.rfilename.ends_with('/') || file.rfilename.is_empty() {
                continue;
            }

            if self.download_cancel.load(Ordering::Relaxed) {
                return Err("下载已取消".to_string());
            }

            let file_url = format!(
                "https://huggingface.co/{}/resolve/main/{}",
                repo_id, file.rfilename
            );
            let file_path = cache_dir.join(&file.rfilename);

            // Skip if file already exists and size matches.
            if let Ok(meta) = std::fs::metadata(&file_path) {
                let expected = file.size.unwrap_or(0);
                if meta.len() == expected || (expected == 0 && meta.len() > 0) {
                    downloaded_bytes += meta.len();
                    continue;
                }
            }

            // Create parent directories for the file.
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("创建目录 {} 失败: {}", parent.display(), e))?;
            }

            // Download with streaming and progress.
            let resp = client
                .get(&file_url)
                .timeout(std::time::Duration::from_secs(600))
                .send()
                .await
                .map_err(|e| format!("下载 {} 失败: {}", file.rfilename, e))?;

            if !resp.status().is_success() {
                return Err(format!(
                    "下载 {} 失败 (HTTP {})",
                    file.rfilename,
                    resp.status()
                ));
            }

            let mut file_out = std::fs::File::create(&file_path)
                .map_err(|e| format!("创建文件 {} 失败: {}", file_path.display(), e))?;
            let mut downloaded: u64 = 0;

            let mut stream = resp.bytes_stream();
            use futures_util::StreamExt;
            let mut last_emit = Instant::now();
            while let Some(chunk) = stream.next().await {
                if self.download_cancel.load(Ordering::Relaxed) {
                    return Err("下载已取消".to_string());
                }
                let chunk = chunk.map_err(|e| format!("下载数据失败: {}", e))?;
                file_out.write_all(&chunk).map_err(|e| {
                    format!("写入文件 {} 失败: {}", file_path.display(), e)
                })?;
                downloaded += chunk.len() as u64;

                // Throttle progress emission to once per second.
                if last_emit.elapsed().as_millis() < 1000 {
                    continue;
                }
                last_emit = Instant::now();

                // Report overall progress.
                let overall = if total_size > 0 {
                    (downloaded_bytes + downloaded) as f32 / total_size as f32
                } else {
                    (i + 1) as f32 / info.siblings.len() as f32
                };
                let elapsed = overall_start.elapsed().as_secs_f64();
                let speed = if elapsed > 0.5 {
                    (downloaded_bytes + downloaded) as f64 / elapsed
                } else {
                    0.0
                } as u64;
                let _ = app.emit(
                    "model-download-progress",
                    DownloadProgressDto {
                        model_name: model_name.to_string(),
                        progress: overall.min(1.0),
                        speed: format_speed(speed),
                        stage: "downloading".to_string(),
                    },
                );
            }

            downloaded_bytes += downloaded;
        }

        let _ = app.emit(
            "model-download-progress",
            DownloadProgressDto {
                model_name: model_name.to_string(),
                progress: 1.0,
                speed: "".to_string(),
                stage: "completed".to_string(),
            },
        );
        Ok(())
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

    /// Check whether the bundled Python + mineru-api is ready.
    pub fn check_python_environment() -> Result<bool, String> {
        Ok(is_bundled_python_ready())
    }

    /// Download and set up a portable Python runtime with mineru-api installed.
    pub async fn setup_python_environment(app: &tauri::AppHandle) -> Result<(), String> {
        let python_dir = python_dir();
        if python_dir.join("python.exe").exists() {
            // Already set up — verify mineru-api is installed.
            let status = Self::run_pip_list(&python_dir).await?;
            if status.contains("mineru") {
                return Ok(());
            }
        }

        // 1. Download python-build-standalone.
        let url = "https://github.com/astral-sh/python-build-standalone/releases/download/20240713/cpython-3.12.5+20240713-x86_64-pc-windows-msvc-install_only.tar.gz";
        let _ = app.emit("python-setup-progress", PythonSetupProgressDto {
            stage: "downloading".to_string(),
            progress: 0.0,
            detail: "正在下载 Python 运行时…".to_string(),
        });

        let client = reqwest::Client::builder()
            .user_agent("OmniMD/0.1.0")
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        let resp = client.get(url).send().await
            .map_err(|e| format!("无法下载 Python 运行时: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("下载 Python 失败 (HTTP {})", resp.status()));
        }

        let total_size = resp.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut stream = resp.bytes_stream();
        use futures_util::StreamExt;
        let mut last_emit = Instant::now();

        // Stream to temp file, then extract.
        let tmp_dir = python_dir.parent().unwrap_or_else(|| std::path::Path::new("."));
        let tmp_path = tmp_dir.join("python-download.tar.gz");
        let mut tmp_file = std::fs::File::create(&tmp_path)
            .map_err(|e| format!("创建临时文件失败: {}", e))?;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("下载数据失败: {}", e))?;
            tmp_file.write_all(&chunk).map_err(|e| format!("写入临时文件失败: {}", e))?;
            downloaded += chunk.len() as u64;

            if last_emit.elapsed().as_millis() >= 1000 && total_size > 0 {
                last_emit = Instant::now();
                let _ = app.emit("python-setup-progress", PythonSetupProgressDto {
                    stage: "downloading".to_string(),
                    progress: downloaded as f32 / total_size as f32,
                    detail: format!("正在下载 Python 运行时 ({:.0}%)", downloaded as f64 / total_size as f64 * 100.0),
                });
            }
        }

        // 2. Extract tar.gz to python directory.
        let _ = app.emit("python-setup-progress", PythonSetupProgressDto {
            stage: "extracting".to_string(),
            progress: 0.0,
            detail: "正在解压 Python 运行时…".to_string(),
        });

        // Remove old python dir if exists.
        if python_dir.exists() {
            std::fs::remove_dir_all(&python_dir)
                .map_err(|e| format!("清理旧 Python 目录失败: {}", e))?;
        }

        let tar_gz = std::fs::File::open(&tmp_path)
            .map_err(|e| format!("打开临时文件失败: {}", e))?;
        let decoder = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(python_dir.parent().unwrap_or_else(|| std::path::Path::new(".")))
            .map_err(|e| format!("解压 Python 失败: {}", e))?;

        // Clean up temp file.
        let _ = std::fs::remove_file(&tmp_path);

        // The tarball extracts to a subdirectory like `python/install/`.
        // Move contents from `python/install/` to `python/` if needed.
        let install_sub = python_dir.join("install");
        if install_sub.exists() {
            let python_dir = python_dir.clone();
            // Move all files from install_sub to python_dir.
            for entry in std::fs::read_dir(&install_sub).map_err(|e| format!("读取目录失败: {}", e))? {
                let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
                let file_name = entry.file_name();
                let src = entry.path();
                let dst = python_dir.join(&file_name);
                // If dst exists, remove it first.
                if dst.exists() {
                    if dst.is_dir() {
                        std::fs::remove_dir_all(&dst).ok();
                    } else {
                        std::fs::remove_file(&dst).ok();
                    }
                }
                std::fs::rename(&src, &dst)
                    .map_err(|e| format!("移动文件失败: {}", e))?;
            }
            std::fs::remove_dir_all(&install_sub).ok();
        }

        let python_exe = python_dir.join("python.exe");
        if !python_exe.exists() {
            return Err("Python 运行时解压后未找到 python.exe".to_string());
        }

        // 3. Install/upgrade pip and install mineru-api.
        let _ = app.emit("python-setup-progress", PythonSetupProgressDto {
            stage: "installing".to_string(),
            progress: 0.0,
            detail: "正在安装 mineru-api…".to_string(),
        });

        // Run pip install mineru-api (with upgrade).
        let output = std::process::Command::new(&python_exe)
            .arg("-m")
            .arg("pip")
            .arg("install")
            .arg("--upgrade")
            .arg("mineru")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|e| format!("无法运行 pip install: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("安装 mineru-api 失败: {}", stderr));
        }

        let _ = app.emit("python-setup-progress", PythonSetupProgressDto {
            stage: "completed".to_string(),
            progress: 1.0,
            detail: "Python 运行时安装完成".to_string(),
        });

        Ok(())
    }

    async fn run_pip_list(python_dir: &PathBuf) -> Result<String, String> {
        let python_exe = python_dir.join("python.exe");
        let output = std::process::Command::new(&python_exe)
            .arg("-m")
            .arg("pip")
            .arg("list")
            .arg("--format=columns")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|e| format!("无法运行 pip list: {}", e))?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
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