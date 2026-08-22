pub mod models;
pub mod engine;
pub mod file_utils;
pub mod markdown_pipeline;
pub mod web_extractor;
pub mod db;

use std::sync::Mutex;
use std::sync::Arc;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::time::Duration;

use db::{
    db as db_handle, DocumentDto, FolderDto, ScanResultDto, SearchHitDto, WorkspaceDto,
};
use engine::batch_queue::BatchQueue;
use engine::mineru_engine::MinerUEngine;
use engine::mineru_runtime::MinerURuntime;
use engine::model_manager::{self, ModelManager};
use engine::DocumentEngine;
use models::ocr::{Cancellation, ProgressCallback};
use models::task::{
    AiReadyOpts, BatchSummaryDto, BatchTaskDto, ConversionError, ConversionResult, ConversionTask,
    ErrorCode, TaskStatus, OutputMode, ParseQuality,
};
use models::ConversionStage;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConversionStatsDto {
    pub image_count: usize,
    pub table_count: usize,
    pub word_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiReadyOptsDto {
    #[serde(default)]
    pub gen_toc: bool,
    #[serde(default)]
    pub gen_meta: bool,
}

impl From<&AiReadyOptsDto> for AiReadyOpts {
    fn from(dto: &AiReadyOptsDto) -> Self {
        AiReadyOpts {
            gen_toc: dto.gen_toc,
            gen_meta: dto.gen_meta,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionResultDto {
    pub task_id: String,
    pub markdown: String,
    pub document_serialized: String,
    pub asset_count: usize,
    pub errors: Vec<ErrorDto>,
    pub success: bool,
    pub output_path: String,
    pub stats: ConversionStatsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDto {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

/// Progress of the one-time environment preparation (Python + model + MinerU).
/// Emitted on `env-prepare-progress` while `prepare_environment` runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvPrepareProgressDto {
    /// One of: `python` | `model` | `mineru` | `done` | `error`.
    pub stage: String,
    pub progress: f32,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgressDto {
    pub task_id: String,
    pub progress: f32,
    pub stage: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusDto {
    pub task_id: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConverterInfo {
    pub name: String,
    pub supported_formats: Vec<String>,
}

struct AppState {
    tasks: Mutex<HashMap<String, ConversionTask>>,
    cancellations: Mutex<HashMap<String, Cancellation>>,
    runtime: Mutex<Option<Arc<MinerURuntime>>>,
    queue_engine: Mutex<Option<Arc<dyn DocumentEngine>>>,
    batch_queue: BatchQueue,
    model_manager: ModelManager,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            tasks: Mutex::new(HashMap::new()),
            cancellations: Mutex::new(HashMap::new()),
            runtime: Mutex::new(None),
            queue_engine: Mutex::new(None),
            batch_queue: BatchQueue::new(3),
            model_manager: ModelManager::new(),
        }
    }
}

impl AppState {
    /// Lazily create the MinerU runtime bound to a stable loopback port.
    fn mineru_runtime(&self) -> Arc<MinerURuntime> {
        let mut guard = self.runtime.lock().unwrap();
        if let Some(r) = guard.as_ref() {
            return r.clone();
        }
        let runtime = Arc::new(MinerURuntime::new(18628, model_manager::install_dir()));
        *guard = Some(runtime.clone());
        runtime
    }

    /// Create a local MinerU engine sharing the lazily created runtime.
    fn create_engine(&self) -> Arc<dyn DocumentEngine> {
        Arc::new(MinerUEngine::new(self.mineru_runtime()))
    }

    /// Lazily create and cache the engine for the batch queue.
    fn queue_engine(&self) -> Arc<dyn DocumentEngine> {
        let mut guard = self.queue_engine.lock().unwrap();
        if let Some(e) = guard.as_ref() {
            return e.clone();
        }
        let engine = self.create_engine();
        *guard = Some(engine.clone());
        engine
    }
}

fn result_to_dto(result: &ConversionResult) -> ConversionResultDto {
    let stats = match &result.stats {
        Some(s) => ConversionStatsDto {
            image_count: s.image_count,
            table_count: s.table_count,
            word_count: s.word_count,
        },
        None => ConversionStatsDto::default(),
    };
    ConversionResultDto {
        task_id: result.task_id.clone(),
        markdown: result.markdown.clone(),
        document_serialized: serde_json::to_string(&result.document).unwrap_or_default(),
        asset_count: result.assets.len(),
        errors: result.errors.iter().map(error_to_dto).collect(),
        success: result.errors.is_empty(),
        output_path: result.output_path.clone(),
        stats,
    }
}

fn error_to_dto(err: &ConversionError) -> ErrorDto {
    ErrorDto {
        code: format!("{:?}", err.code),
        message: err.message.clone(),
        retryable: err.retryable,
    }
}

fn emit_progress(app: &tauri::AppHandle, task: &ConversionTask) {
    let _ = app.emit(
        "task-progress",
        TaskProgressDto {
            task_id: task.id.clone(),
            progress: task.progress,
            stage: format!("{:?}", task.stage),
            detail: None,
        },
    );
}

fn emit_status(app: &tauri::AppHandle, task: &ConversionTask) {
    let _ = app.emit(
        "task-status",
        TaskStatusDto {
            task_id: task.id.clone(),
            status: format!("{:?}", task.status),
            error: task.error.clone(),
        },
    );
}

fn get_state(app: &tauri::AppHandle) -> Result<tauri::State<'_, AppState>, String> {
    app.try_state().ok_or_else(|| "Application state not available".to_string())
}

#[tauri::command]
async fn convert_file(
    app: tauri::AppHandle,
    source_path: String,
    output_dir: String,
    output_mode: Option<String>,
    ai_ready_opts: Option<AiReadyOptsDto>,
    parse_quality: Option<String>,
    client_task_id: Option<String>,
) -> Result<ConversionResultDto, String> {
    info!(
        "convert_file: {} -> {} (mode={:?}, quality={:?})",
        source_path, output_dir, output_mode, parse_quality
    );

    let mode = output_mode
        .as_deref()
        .map(OutputMode::from_str)
        .unwrap_or_default();
    let quality = parse_quality
        .as_deref()
        .map(ParseQuality::from_str)
        .unwrap_or_default();

    let output_path = file_utils::get_output_path(&source_path, &output_dir);
    let mut task = ConversionTask::with_mode(&source_path, &output_path, mode);
    // When the frontend supplies its own task id (used for the session list and
    // cancellation), use it so progress/status events can be correlated.
    if let Some(id) = client_task_id {
        if !id.trim().is_empty() {
            task.id = id;
        }
    }
    if let Some(dto) = &ai_ready_opts {
        task.ai_ready_opts = AiReadyOpts::from(dto);
    }
    task.parse_quality = quality;
    task.status = TaskStatus::Processing;
    task.stage = ConversionStage::Queued;
    task.progress = 0.05;

    let state = get_state(&app)?;
    let cancellation = Cancellation::new();
    state.cancellations.lock().unwrap().insert(task.id.clone(), cancellation.clone());
    state.tasks.lock().unwrap().insert(task.id.clone(), task.clone());
    emit_progress(&app, &task);

    // Create a progress callback that emits events to the frontend.
    let app_clone = app.clone();
    let task_id = task.id.clone();
    let progress_cb: ProgressCallback = Arc::new(move |p: f32, detail: Option<String>| {
        let _ = app_clone.emit(
            "task-progress",
            TaskProgressDto {
                task_id: task_id.clone(),
                progress: p,
                stage: if p < 0.35 {
                    "ModelLoading".to_string()
                } else if p < 0.9 {
                    "Parsing".to_string()
                } else {
                    "Saving".to_string()
                },
                detail,
            },
        );
    });

    let engine: Arc<dyn DocumentEngine> = state.create_engine();
    let result = match engine
        .convert(&task, Some(progress_cb), Some(&cancellation))
        .await
    {
        Ok(r) => r,
        Err(e) => {
            if e.code == ErrorCode::Cancelled || cancellation.cancelled() {
                task.status = TaskStatus::Cancelled;
                task.error = Some("任务已取消".to_string());
                emit_status(&app, &task);
                cleanup_cancellation(&state, &task.id);
                cleanup_task(&state, &task.id);
                return Err("cancelled".to_string());
            }
            task.status = TaskStatus::Failed;
            task.error = Some(e.message.clone());
            emit_status(&app, &task);
            cleanup_cancellation(&state, &task.id);
            cleanup_task(&state, &task.id);
            return Err(format!("[{:?}]: {}", e.code, e.message));
        }
    };

    task.status = TaskStatus::Completed;
    task.progress = 1.0;
    emit_progress(&app, &task);
    emit_status(&app, &task);
    cleanup_cancellation(&state, &task.id);
    cleanup_task(&state, &task.id);

    Ok(result_to_dto(&result))
}

/// Remove a task's cancellation entry from state after the conversion ends.
fn cleanup_cancellation(state: &tauri::State<'_, AppState>, task_id: &str) {
    state
        .cancellations
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(task_id);
}

/// Remove a completed/failed/cancelled task from the tasks map to prevent
/// unbounded memory growth. The frontend receives the result directly via the
/// command return value, so the map entry is no longer needed after the task
/// reaches a terminal state.
fn cleanup_task(state: &tauri::State<'_, AppState>, task_id: &str) {
    state
        .tasks
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(task_id);
}

/// Request cancellation of a running conversion task. The backend cooperatively
/// stops at the next checkpoint.
#[tauri::command]
async fn cancel_task(app: tauri::AppHandle, task_id: String) -> Result<(), String> {
    let state = get_state(&app)?;
    let mut cancelled = false;
    {
        let guard = state.cancellations.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cancellation) = guard.get(&task_id) {
            cancellation.cancel();
            cancelled = true;
        }
    }
    if cancelled {
        tracing::info!("Cancel requested for task {}", task_id);
    } else {
        // Not a single-file task: fall back to the batch queue so the same API
        // can cancel batch tasks (avoids frontend having to pick the right one).
        let _ = state.batch_queue.cancel_task(&app, &task_id).await;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// M2 Workbench data layer commands (SQLite workspace DB)
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_workspaces(app: tauri::AppHandle) -> Result<Vec<WorkspaceDto>, String> {
    db_handle(&app)?.list_workspaces()
}

#[tauri::command]
fn add_workspace(
    app: tauri::AppHandle,
    name: String,
    path: String,
) -> Result<WorkspaceDto, String> {
    db_handle(&app)?.add_workspace(&name, &path)
}

#[tauri::command]
fn remove_workspace(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    db_handle(&app)?.remove_workspace(id)
}

#[tauri::command]
fn get_active_workspace(app: tauri::AppHandle) -> Result<Option<WorkspaceDto>, String> {
    let handle = db_handle(&app)?;
    match handle.get_active_workspace_id()? {
        Some(id) => handle.get_workspace(id),
        None => Ok(None),
    }
}

#[tauri::command]
fn set_active_workspace(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    db_handle(&app)?.set_active_workspace_id(id)
}

#[tauri::command]
fn scan_workspace(app: tauri::AppHandle, id: i64) -> Result<ScanResultDto, String> {
    db_handle(&app)?.scan_workspace(id)
}

#[tauri::command]
fn list_documents(
    app: tauri::AppHandle,
    workspace_id: i64,
    folder: Option<String>,
) -> Result<Vec<DocumentDto>, String> {
    db_handle(&app)?.list_documents(workspace_id, folder.as_deref())
}

#[tauri::command]
fn list_subfolders(
    app: tauri::AppHandle,
    workspace_id: i64,
    folder: Option<String>,
) -> Result<Vec<FolderDto>, String> {
    db_handle(&app)?.list_subfolders(workspace_id, folder.as_deref())
}

#[tauri::command]
fn list_favorites(app: tauri::AppHandle, workspace_id: i64) -> Result<Vec<DocumentDto>, String> {
    db_handle(&app)?.list_favorites(workspace_id)
}

#[tauri::command]
fn list_recent(app: tauri::AppHandle, workspace_id: Option<i64>, limit: Option<i64>) -> Result<Vec<DocumentDto>, String> {
    db_handle(&app)?.list_recent(workspace_id, limit.unwrap_or(20))
}

#[tauri::command]
fn set_document_favorite(
    app: tauri::AppHandle,
    id: i64,
    favorite: bool,
) -> Result<(), String> {
    db_handle(&app)?.set_favorite(id, favorite)
}

#[tauri::command]
fn record_document_open(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    db_handle(&app)?.record_open(id)
}

#[tauri::command]
fn search_documents(
    app: tauri::AppHandle,
    query: String,
    workspace_id: i64,
    limit: Option<i64>,
) -> Result<Vec<SearchHitDto>, String> {
    db_handle(&app)?.search(&query, workspace_id, limit.unwrap_or(50))
}

/// Check whether the bundled Python runtime is ready for mineru-api.
#[tauri::command]
async fn check_python_environment() -> Result<bool, String> {
    ModelManager::check_python_environment()
}

/// Download and set up a portable Python + mineru-api.
#[tauri::command]
async fn setup_python_environment(app: tauri::AppHandle) -> Result<(), String> {
    ModelManager::setup_python_environment(&app).await
}

/// Start the MinerU runtime and wait until it is healthy. Returns engine info.
#[tauri::command]
async fn start_mineru(app: tauri::AppHandle) -> Result<String, String> {
    let state = get_state(&app)?;
    let runtime = state.mineru_runtime();
    runtime.ensure_running().await?;
    Ok(format!("MinerU 服务已就绪: {}", runtime.base_url))
}

/// Probe the MinerU runtime health without starting it.
#[tauri::command]
async fn mineru_status(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let state = get_state(&app)?;
    let runtime = state.mineru_runtime();
    let healthy = runtime.is_healthy().await;
    Ok(serde_json::json!({
        "healthy": healthy,
        "baseUrl": runtime.base_url,
    }))
}

/// One-time environment preparation that makes the app usable out of the box:
/// ensure the Python runtime (bundled or auto-installed), download the default
/// pipeline model if missing, and start the MinerU engine. Runs in the
/// background and reports progress via `env-prepare-progress`; the frontend
/// never needs to prompt the user to click "install" or "download".
#[tauri::command]
async fn prepare_environment(app: tauri::AppHandle) -> Result<(), String> {
    let runtime = get_state(&app)?.mineru_runtime();
    let model_mgr = ModelManager::new();
    let app_for_task = app.clone();

    tauri::async_runtime::spawn(async move {
        let emit = |stage: &str, progress: f32, detail: &str| {
            let _ = app_for_task.emit(
                "env-prepare-progress",
                EnvPrepareProgressDto {
                    stage: stage.to_string(),
                    progress,
                    detail: detail.to_string(),
                },
            );
        };

        emit("python", 0.02, "正在准备 Python 运行环境…");
        if let Err(e) = ModelManager::setup_python_environment(&app_for_task).await {
            emit("error", 0.0, &format!("Python 环境准备失败：{e}"));
            return;
        }

        emit("model", 0.1, "正在准备解析模型…");
        let needs_model = match model_mgr.get_model_status("pipeline").await {
            Ok(m) => m.status != "downloaded",
            Err(_) => true,
        };
        if needs_model {
            if let Err(e) = model_mgr.download_model(&app_for_task, "pipeline").await {
                emit("error", 0.0, &format!("模型下载失败：{e}"));
                return;
            }
        }

        emit("mineru", 0.97, "正在启动 MinerU 引擎…");
        if let Err(e) = runtime.ensure_running().await {
            emit("error", 0.0, &format!("MinerU 启动失败：{e}"));
            return;
        }

        emit("done", 1.0, "环境准备完成");
    });

    Ok(())
}

#[tauri::command]
async fn fetch_url(
    app: tauri::AppHandle,
    url: String,
    output_dir: String,
    output_mode: Option<String>,
    ai_ready_opts: Option<AiReadyOptsDto>,
    client_task_id: Option<String>,
) -> Result<ConversionResultDto, String> {
    info!("fetch_url: {} -> {} (mode={:?})", url, output_dir, output_mode);

    let mode = output_mode
        .as_deref()
        .map(OutputMode::from_str)
        .unwrap_or_default();

    let filename = web_extractor::derive_filename(&url, "");
    let output_path = format!("{}/{}.md", output_dir.trim_end_matches('/'), filename);
    let mut task = ConversionTask::with_mode(&url, &output_path, mode.clone());
    if let Some(id) = client_task_id {
        if !id.trim().is_empty() {
            task.id = id;
        }
    }
    if let Some(dto) = &ai_ready_opts {
        task.ai_ready_opts = AiReadyOpts::from(dto);
    }
    task.status = TaskStatus::Processing;
    task.stage = ConversionStage::Fetching;
    task.progress = 0.1;

    let state = get_state(&app)?;
    let cancellation = Cancellation::new();
    state.cancellations.lock().unwrap().insert(task.id.clone(), cancellation.clone());
    state.tasks.lock().unwrap().insert(task.id.clone(), task.clone());
    emit_progress(&app, &task);

    // Helper to check cancellation and clean up if cancelled.
    let check_cancelled = |task: &mut ConversionTask, state: &tauri::State<'_, AppState>| -> Result<(), String> {
        if cancellation.cancelled() {
            task.status = TaskStatus::Cancelled;
            task.error = Some("任务已取消".to_string());
            emit_status(&app, task);
            cleanup_cancellation(state, &task.id);
            cleanup_task(state, &task.id);
            return Err("cancelled".to_string());
        }
        Ok(())
    };

    check_cancelled(&mut task, &state)?;

    task.stage = ConversionStage::Fetching;
    task.progress = 0.3;
    emit_progress(&app, &task);

    let html = web_extractor::fetch_html(&url).await?;

    check_cancelled(&mut task, &state)?;

    task.stage = ConversionStage::Parsing;
    task.progress = 0.5;
    emit_progress(&app, &task);

    let extracted = web_extractor::extract_content(&html, &url)
        .map_err(|e| format!("Failed to extract content: {}", e))?;

    task.stage = ConversionStage::PostProcessing;
    task.progress = 0.7;
    emit_progress(&app, &task);

    check_cancelled(&mut task, &state)?;

    let markdown = markdown_pipeline::process(
        &extracted.markdown,
        &mode,
        &url,
        &task.ai_ready_opts,
    );

    let output_dir_path = std::path::Path::new(&output_dir);
    if !output_dir_path.exists() {
        std::fs::create_dir_all(output_dir_path).map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    check_cancelled(&mut task, &state)?;

    task.stage = ConversionStage::Saving;
    task.progress = 0.9;
    emit_progress(&app, &task);

    std::fs::write(&output_path, &markdown)
        .map_err(|e| format!("Failed to write markdown: {}", e))?;

    task.status = TaskStatus::Completed;
    task.progress = 1.0;
    emit_progress(&app, &task);
    emit_status(&app, &task);
    cleanup_cancellation(&state, &task.id);
    cleanup_task(&state, &task.id);

    let table_count = markdown_pipeline::count_table_separators(&markdown);
    let word_count = markdown_pipeline::count_words(&markdown);
    let result = ConversionResultDto {
        task_id: task.id.clone(),
        markdown,
        document_serialized: String::new(),
        asset_count: 0,
        errors: vec![],
        success: true,
        output_path,
        stats: ConversionStatsDto {
            image_count: 0,
            table_count,
            word_count,
        },
    };

    Ok(result)
}

/// Return the default output directory: the directory that contains the
/// running binary, with an `output/` subdirectory appended.
#[tauri::command]
fn get_default_output_dir() -> String {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let install_dir = exe.parent().unwrap_or(exe.as_path());
    let out = install_dir.join("output");
    out.to_string_lossy().replace("\\", "/").to_string()
}

/// Open the given directory in the system file manager.
#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .arg("/c")
            .arg("start")
            .arg("")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn get_supported_formats() -> Vec<String> {
    file_utils::get_supported_extensions()
}

#[tauri::command]
fn get_converter_info() -> String {
    let info = ConverterInfo {
        name: "MinerU 3.x".to_string(),
        supported_formats: get_supported_formats(),
    };
    serde_json::to_string(&info).unwrap_or_default()
}

#[tauri::command]
fn get_app_version(app: tauri::AppHandle) -> String {
    format!("v{}", app.package_info().version)
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    fs::write(&path, &content).map_err(|e| format!("写入文件失败: {}", e))
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {}", e))
}

#[tauri::command]
fn list_files_in_folder(path: String) -> Result<Vec<String>, String> {
    let files = file_utils::list_files_flat(&path, file_utils::get_supported_extensions_ref());
    Ok(files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
async fn download_url(url: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("OmniMD/0.1.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("服务器返回错误状态码: {}", response.status()));
    }

    // Accept only binary content types that represent downloadable files.
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let is_acceptable = content_type.is_empty()
        || content_type.starts_with("application/octet-stream")
        || content_type.starts_with("application/pdf")
        || content_type.starts_with("application/vnd")
        || content_type.starts_with("image/")
        || content_type.starts_with("text/")
        || content_type.starts_with("application/zip")
        || content_type.starts_with("application/xml")
        || content_type.starts_with("application/json");
    if !is_acceptable {
        return Err(format!("不支持的内容类型: {}", content_type));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;

    let temp_dir = std::env::temp_dir().join("omnimd_downloads");
    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("创建临时目录失败: {}", e))?;

    let filename = url
        .split('/')
        .last()
        .filter(|s| !s.is_empty() && s.contains('.'))
        .unwrap_or("downloaded_file.bin")
        .to_string();

    let sanitized: String = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect();
    let sanitized = if sanitized.is_empty() {
        "downloaded_file.bin".to_string()
    } else {
        sanitized
    };

    let file_path = temp_dir.join(&sanitized);
    fs::write(&file_path, &bytes)
        .map_err(|e| format!("保存文件失败: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Batch task commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn batch_enqueue(
    app: tauri::AppHandle,
    source_path: String,
    output_path: String,
    output_mode: Option<String>,
    parse_quality: Option<String>,
) -> Result<String, String> {
    let mode = output_mode
        .as_deref()
        .map(OutputMode::from_str)
        .unwrap_or_default();
    let quality = parse_quality
        .as_deref()
        .map(ParseQuality::from_str)
        .unwrap_or_default();
    let state = get_state(&app)?;
    state
        .batch_queue
        .enqueue(app.clone(), source_path, output_path, mode, quality)
        .await
}

#[tauri::command]
async fn batch_start(app: tauri::AppHandle) -> Result<(), String> {
    let state = get_state(&app)?;
    let engine = state.queue_engine();
    state.batch_queue.start(app.clone(), engine).await;
    Ok(())
}

#[tauri::command]
async fn batch_pause_task(app: tauri::AppHandle, task_id: String) -> Result<(), String> {
    let state = get_state(&app)?;
    state.batch_queue.pause_task(&app, &task_id).await
}

#[tauri::command]
async fn batch_resume_task(app: tauri::AppHandle, task_id: String) -> Result<(), String> {
    let state = get_state(&app)?;
    let engine = state.queue_engine();
    state.batch_queue.resume_task(&app, engine, &task_id).await
}

#[tauri::command]
async fn batch_cancel_task(app: tauri::AppHandle, task_id: String) -> Result<(), String> {
    let state = get_state(&app)?;
    state.batch_queue.cancel_task(&app, &task_id).await
}

#[tauri::command]
async fn batch_cancel_all(app: tauri::AppHandle) -> Result<(), String> {
    let state = get_state(&app)?;
    state.batch_queue.cancel_all(&app).await
}

#[tauri::command]
async fn batch_retry_task(app: tauri::AppHandle, task_id: String) -> Result<(), String> {
    let state = get_state(&app)?;
    let engine = state.queue_engine();
    state.batch_queue.retry_task(&app, engine, &task_id).await
}

#[tauri::command]
async fn batch_retry_failed(app: tauri::AppHandle) -> Result<(), String> {
    let state = get_state(&app)?;
    let engine = state.queue_engine();
    state.batch_queue.retry_failed(&app, engine).await
}

#[tauri::command]
async fn batch_clear_done(app: tauri::AppHandle) -> Result<(), String> {
    let state = get_state(&app)?;
    state.batch_queue.clear_done(&app).await
}

#[tauri::command]
fn batch_set_concurrency(app: tauri::AppHandle, concurrency: u32) -> Result<(), String> {
    let state = get_state(&app)?;
    state.batch_queue.set_concurrency(concurrency);
    Ok(())
}

#[tauri::command]
fn batch_list_tasks(app: tauri::AppHandle) -> Result<Vec<BatchTaskDto>, String> {
    use db::db as db_handle;
    db_handle(&app)?.list_all_batch_tasks()
}

#[tauri::command]
fn batch_get_summary(app: tauri::AppHandle) -> Result<BatchSummaryDto, String> {
    use db::db as db_handle;
    db_handle(&app)?.get_batch_summary()
}


/// Whether the local pipeline model has been downloaded (non-empty
/// `models/pipeline` directory). Used to decide when to show the first-launch
/// download banner.
#[tauri::command]
async fn is_model_downloaded(app: tauri::AppHandle) -> Result<bool, String> {
    let state = get_state(&app)?;
    let models = state.model_manager.list_models().await?;
    Ok(models
        .iter()
        .any(|m| m.name == "pipeline" && m.status == "downloaded"))
}

// ---------------------------------------------------------------------------
// Model management commands
// ---------------------------------------------------------------------------

use engine::model_manager::{ModelInfoDto, CacheInfoDto};

#[tauri::command]
async fn list_models(app: tauri::AppHandle) -> Result<Vec<ModelInfoDto>, String> {
    let state = get_state(&app)?;
    state.model_manager.list_models().await
}

#[tauri::command]
async fn get_model_status(app: tauri::AppHandle, model_name: String) -> Result<ModelInfoDto, String> {
    let state = get_state(&app)?;
    state.model_manager.get_model_status(&model_name).await
}

#[tauri::command]
async fn download_model(app: tauri::AppHandle, model_name: String) -> Result<(), String> {
    let state = get_state(&app)?;
    state.model_manager.download_model(&app, &model_name).await
}

#[tauri::command]
async fn cancel_model_download(app: tauri::AppHandle) -> Result<(), String> {
    let state = get_state(&app)?;
    state.model_manager.cancel_download(&app).await
}

#[tauri::command]
async fn get_cache_info(app: tauri::AppHandle) -> Result<CacheInfoDto, String> {
    let state = get_state(&app)?;
    state.model_manager.get_cache_info().await
}

#[tauri::command]
async fn clear_model_cache(app: tauri::AppHandle) -> Result<(), String> {
    let state = get_state(&app)?;
    state.model_manager.clear_cache().await
}

#[tauri::command]
async fn set_model_source(app: tauri::AppHandle, source: String) -> Result<(), String> {
    let state = get_state(&app)?;
    state.model_manager.set_source(source).await
}

#[tauri::command]
async fn get_model_source(app: tauri::AppHandle) -> Result<String, String> {
    let state = get_state(&app)?;
    state.model_manager.get_source().await
}

#[tauri::command]
async fn import_offline_model(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let state = get_state(&app)?;
    state.model_manager.import_offline(&app, &path).await
}

#[tauri::command]
async fn check_model_update(app: tauri::AppHandle, model_name: String) -> Result<bool, String> {
    let state = get_state(&app)?;
    state.model_manager.check_update(&model_name).await
}

/// Initialize a tracing subscriber that writes to a log file under the
/// user-writable AppData/Roaming/OmniMD/logs directory (falling back to stdout
/// if that file cannot be created). This makes runtime diagnostics — including
/// the captured `mineru-api` stdout/stderr — inspectable instead of being
/// silently discarded in a bundled build.
fn init_logging() {
    use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*};

    let log_dir = std::env::var("APPDATA")
        .map(|p| std::path::PathBuf::from(p).join("OmniMD").join("logs"))
        .unwrap_or_else(|_| std::env::temp_dir().join("omnimd_logs"));
    let _ = std::fs::create_dir_all(&log_dir);
    let file = std::fs::File::create(log_dir.join("omnimd.log")).ok();

    // BoxMakeWriter unifies the two writer types so both match arms share one
    // concrete type.
    use tracing_subscriber::fmt::writer::BoxMakeWriter;
    let writer: BoxMakeWriter = match file {
        Some(f) => BoxMakeWriter::new(f),
        None => BoxMakeWriter::new(io::stdout),
    };

    let layer = fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_filter(LevelFilter::INFO);

    let _ = tracing_subscriber::registry().with(layer).try_init();
}

/// Capture panics (which otherwise only go to stderr and are discarded in a
/// bundled app) into a dedicated log file so a hard crash is diagnosable.
fn install_panic_hook() {
    let log_dir = std::env::var("APPDATA")
        .map(|p| std::path::PathBuf::from(p).join("OmniMD").join("logs"))
        .unwrap_or_else(|_| std::env::temp_dir().join("omnimd_logs"));
    let _ = std::fs::create_dir_all(&log_dir);
    let panic_file = log_dir.join("omnimd.panic.log");
    std::panic::set_hook(Box::new(move |info| {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string payload>".to_string()
        };
        let msg = format!(
            "[{}] PANIC at {}\n  message: {}\n  thread: {}\n",
            ts,
            loc,
            payload,
            std::thread::current().name().unwrap_or("unnamed")
        );
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&panic_file)
            .and_then(|mut f| std::io::Write::write_all(&mut f, msg.as_bytes()));
    }));
}

pub fn run() {
    // Capture hard crashes before anything else.
    install_panic_hook();
    // Initialize logging first so all subsequent diagnostics are captured.
    init_logging();

    // Clean up leftover temp download files from previous runs.
    let temp_dir = std::env::temp_dir().join("omnimd_downloads");
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // Collect file-path CLI arguments (e.g. from a shell context menu or drag-drop).
    let cli_args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| {
            let p = std::path::Path::new(a);
            p.exists() && p.is_file()
        })
        .collect();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // Forward file-path argv from the shell context menu to the frontend.
            if !cli_args.is_empty() {
                let _ = app.emit("argv-files", &cli_args);
            }
            // Mark stale Processing tasks from a previous session as Failed.
            let _ = db::reconcile_stale_batch_tasks(app.handle());
            Ok(())
        })
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            convert_file,
            fetch_url,
            cancel_task,
            start_mineru,
            mineru_status,
            get_default_output_dir,
            open_folder,
            get_supported_formats,
            get_converter_info,
            get_app_version,
            write_text_file,
            read_text_file,
            list_files_in_folder,
            download_url,
            list_workspaces,
            add_workspace,
            remove_workspace,
            get_active_workspace,
            set_active_workspace,
            scan_workspace,
            list_documents,
            list_subfolders,
            list_favorites,
            list_recent,
            set_document_favorite,
            record_document_open,
            search_documents,
            batch_enqueue,
            batch_start,
            batch_pause_task,
            batch_resume_task,
            batch_cancel_task,
            batch_cancel_all,
            batch_retry_task,
            batch_retry_failed,
            batch_clear_done,
            batch_set_concurrency,
            batch_list_tasks,
            batch_get_summary,
            is_model_downloaded,
            list_models,
            get_model_status,
            download_model,
            cancel_model_download,
            get_cache_info,
            clear_model_cache,
            set_model_source,
            get_model_source,
            import_offline_model,
            check_model_update,
            check_python_environment,
            setup_python_environment,
            prepare_environment,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
