pub mod models;
pub mod engine;
pub mod file_utils;
pub mod markdown_pipeline;
pub mod db;
pub mod text_utils;
pub mod cache;

use std::sync::Mutex;
use std::sync::Arc;
use std::collections::HashMap;
use std::fs;
use std::io;

use db::{
    db as db_handle, DocumentDto, FolderDto, ScanResultDto, SearchHitDto, WorkspaceDto,
};
use engine::batch_queue::BatchQueue;
use engine::anydoc_engine::AnyDocEngine;
use engine::DocumentEngine;
use models::task::{
    BatchSummaryDto, BatchTaskDto, Cancellation, ConversionError, ConversionResult,
    ConversionTask, ErrorCode, ProgressCallback, TaskStatus,
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
    cancellations: Mutex<HashMap<String, Cancellation>>,
    queue_engine: Mutex<Option<Arc<dyn DocumentEngine>>>,
    batch_queue: BatchQueue,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            cancellations: Mutex::new(HashMap::new()),
            queue_engine: Mutex::new(None),
            batch_queue: BatchQueue::new(3),
        }
    }
}

impl AppState {
    /// Create the document conversion engine: the local AnyDoc engine
    /// (pure Rust, no ML models, no external services).
    fn create_engine(&self) -> Arc<dyn DocumentEngine> {
        Arc::new(AnyDocEngine::new())
    }

    /// Lazily create and cache the engine for the batch queue.
    fn queue_engine(&self) -> Arc<dyn DocumentEngine> {
        let mut guard = self.queue_engine.lock().unwrap_or_else(|e| e.into_inner());
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
    client_task_id: Option<String>,
) -> Result<ConversionResultDto, String> {
    info!("convert_file: {} -> {}", source_path, output_dir);

    let output_path = file_utils::get_output_path(&source_path, &output_dir);
    let mut task = ConversionTask::new(&source_path, &output_path);
    // When the frontend supplies its own task id (used for the session list and
    // cancellation), use it so progress/status events can be correlated.
    if let Some(id) = client_task_id {
        if !id.trim().is_empty() {
            task.id = id;
        }
    }
    task.status = TaskStatus::Processing;
    task.stage = ConversionStage::Queued;
    task.progress = 0.05;

    let state = get_state(&app)?;
    let cancellation = Cancellation::new();
    // 两个并发转换可能携带相同的 client_task_id（前端跨会话复用 id），直接
    // 以相同 key insert 会覆盖前一个任务的取消句柄使其永远无法取消。key
    // 冲突时改用新 UUID 作为注册表键，任务对外 id 不变。
    let cancel_key = {
        let guard = state
            .cancellations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if guard.contains_key(&task.id) {
            uuid::Uuid::new_v4().to_string()
        } else {
            task.id.clone()
        }
    };
    state
        .cancellations
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(cancel_key.clone(), cancellation.clone());
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
    // Run the (CPU-bound, synchronous) parse on a blocking thread so it never
    // occupies a tokio worker and starves the rest of the app.
    let engine_task = task.clone();
    let engine_cancel = cancellation.clone();
    let engine_progress = progress_cb.clone();
    let result = match tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(engine.convert(
            &engine_task,
            Some(engine_progress),
            Some(&engine_cancel),
        ))
    })
    .await
    .unwrap_or_else(|e| {
        Err(ConversionError {
            code: ErrorCode::EngineError,
            message: format!("转换任务异常退出: {e}"),
            stage: ConversionStage::Parsing,
            retryable: true,
            page: None,
        })
    })
    {
        Ok(r) => r,
        Err(e) => {
            if e.code == ErrorCode::Cancelled || cancellation.cancelled() {
                task.status = TaskStatus::Cancelled;
                task.error = Some("任务已取消".to_string());
                emit_status(&app, &task);
                cleanup_cancellation(&state, &cancel_key);
                return Err("cancelled".to_string());
            }
            task.status = TaskStatus::Failed;
            task.error = Some(e.message.clone());
            emit_status(&app, &task);
            cleanup_cancellation(&state, &cancel_key);
            return Err(format!("[{:?}]: {}", e.code, e.message));
        }
    };

    task.status = TaskStatus::Completed;
    task.progress = 1.0;
    emit_progress(&app, &task);
    emit_status(&app, &task);
    cleanup_cancellation(&state, &cancel_key);

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
        Ok(())
    } else {
        // Not a single-file task: fall back to the batch queue so the same API
        // can cancel batch tasks (avoids frontend having to pick the right one).
        state.batch_queue.cancel_task(&app, &task_id).await
    }
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

/// Re-index a workspace. Heavy filesystem work runs on a blocking thread:
/// sync commands execute on the main thread, so scanning a large root (an
/// entire drive) there would freeze the whole UI.
#[tauri::command]
async fn scan_workspace(app: tauri::AppHandle, id: i64) -> Result<ScanResultDto, String> {
    tauri::async_runtime::spawn_blocking(move || db::scan_workspace_background(&app, id))
        .await
        .map_err(|e| format!("后台扫描任务异常退出: {e}"))?
}

#[tauri::command]
async fn list_documents(
    app: tauri::AppHandle,
    workspace_id: i64,
    folder: Option<String>,
) -> Result<Vec<DocumentDto>, String> {
    // 移出主线程：大工作区单次加载上千行，且扫描事务提交期间 DB 锁会被
    // 短暂占用，同步命令在主线程执行会冻结 UI。
    tauri::async_runtime::spawn_blocking(move || {
        db_handle(&app)?.list_documents(workspace_id, folder.as_deref())
    })
    .await
    .map_err(|e| format!("后台任务异常退出: {e}"))?
}

#[tauri::command]
async fn list_subfolders(
    app: tauri::AppHandle,
    workspace_id: i64,
    folder: Option<String>,
) -> Result<Vec<FolderDto>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        db_handle(&app)?.list_subfolders(workspace_id, folder.as_deref())
    })
    .await
    .map_err(|e| format!("后台任务异常退出: {e}"))?
}

#[tauri::command]
fn list_favorites(app: tauri::AppHandle, workspace_id: i64) -> Result<Vec<DocumentDto>, String> {
    db_handle(&app)?.list_favorites(workspace_id)
}

#[tauri::command]
fn list_recent(app: tauri::AppHandle, workspace_id: Option<i64>, limit: Option<i64>) -> Result<Vec<DocumentDto>, String> {
    // clamp：SQLite 的 LIMIT 负值语义是"不限制"，会拉全表。
    db_handle(&app)?.list_recent(workspace_id, limit.unwrap_or(20).clamp(1, 500))
}

// Library DB writes run off the main thread: sync commands execute there and
// would block the UI whenever the DB mutex is briefly contended (e.g. a
// workspace scan committing).
#[tauri::command]
async fn set_document_favorite(
    app: tauri::AppHandle,
    id: i64,
    favorite: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let handle = db_handle(&app)?;
        handle.set_favorite(id, favorite)
    })
    .await
    .map_err(|e| format!("后台任务异常退出: {e}"))?
}

#[tauri::command]
async fn record_document_open(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let handle = db_handle(&app)?;
        handle.record_open(id)
    })
    .await
    .map_err(|e| format!("后台任务异常退出: {e}"))?
}

#[tauri::command]
async fn search_documents(
    app: tauri::AppHandle,
    query: String,
    workspace_id: i64,
    limit: Option<i64>,
) -> Result<Vec<SearchHitDto>, String> {
    // Also kept off the main thread: while a big scan transaction commits,
    // the DB mutex can be briefly contended and must never block the UI.
    tauri::async_runtime::spawn_blocking(move || {
        db_handle(&app)?.search(&query, workspace_id, limit.unwrap_or(50).clamp(1, 500))
    })
    .await
    .map_err(|e| format!("检索任务异常退出: {e}"))?
}

/// Return the default output directory. This lives under the app data dir
/// (`%APPDATA%/<identifier>/output` on Windows) rather than next to the
/// executable, because the install directory is not writable for a normal
/// user after an MSI install (Program Files).
#[tauri::command]
fn get_default_output_dir(app: tauri::AppHandle) -> Result<String, String> {
    let base = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    let out = base.join("output");
    // 创建失败必须报错而不是返回一个无效路径，否则后续转换才在半路失败。
    std::fs::create_dir_all(&out).map_err(|e| format!("创建默认输出目录失败: {e}"))?;
    Ok(out.to_string_lossy().replace("\\", "/").to_string())
}

/// Open the given directory in the system file manager.
///
/// The path is passed as a single argument to the platform opener (never
/// through `cmd.exe`), and must resolve to a real directory, so a crafted
/// path cannot inject shell metacharacters or launch an arbitrary program.
#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let p = std::path::Path::new(trimmed);
    let dir = if p.is_dir() {
        p.to_path_buf()
    } else if p.is_file() {
        p.parent().map(|x| x.to_path_buf()).unwrap_or_else(|| p.to_path_buf())
    } else {
        return Err(format!("目录不存在: {}", trimmed));
    };

    #[cfg(target_os = "windows")]
    {
        // `explorer` does not interpret `&`/`|`/`^`; args are passed directly.
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    Ok(())
}

/// Guard for text-file commands: reject NUL bytes and restrict access to
/// Markdown-ish text files. This is defense in depth alongside the CSP and
/// the sanitized Markdown preview.
fn ensure_text_path(path: &str) -> Result<(), String> {
    if path.contains('\0') {
        return Err("非法路径".to_string());
    }
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "md" | "markdown" | "txt") {
        return Err(format!("仅支持 Markdown/文本文件，收到: .{}", ext));
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
        name: "AnyDoc".to_string(),
        supported_formats: get_supported_formats(),
    };
    serde_json::to_string(&info).unwrap_or_default()
}

#[tauri::command]
fn get_app_version(app: tauri::AppHandle) -> String {
    format!("v{}", app.package_info().version)
}

/// 单次写入/读取文本的大小上限：防止异常巨大的内容整块进内存或写盘。
const WRITE_TEXT_MAX_BYTES: usize = 20 * 1024 * 1024;
const READ_TEXT_MAX_BYTES: u64 = 10 * 1024 * 1024;

/// 原子写入：先写 `<path>.tmp` 再 rename，进程在写入中途崩溃也不会留下
/// 截断的目标文件。Windows 上 rename 无法覆盖已存在目标，先删目标再
/// rename（两个操作之间的窗口极短）。
fn write_text_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    fs::write(&tmp, bytes).map_err(|e| format!("写入文件失败: {e}"))?;
    if path.exists() {
        if let Err(e) = fs::remove_file(path) {
            let _ = fs::remove_file(&tmp);
            return Err(format!("替换文件失败: {e}"));
        }
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("写入文件失败: {e}")
    })
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    ensure_text_path(&path)?;
    if content.len() > WRITE_TEXT_MAX_BYTES {
        return Err("内容过大，超出可保存上限（20MB）".to_string());
    }
    write_text_atomic(std::path::Path::new(&path), content.as_bytes())
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    ensure_text_path(&path)?;
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > READ_TEXT_MAX_BYTES {
            return Err("文件过大（超过 10MB），暂不支持在编辑器中打开".to_string());
        }
    }
    fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {e}"))
}

#[tauri::command]
fn list_files_in_folder(path: String) -> Result<Vec<String>, String> {
    if path.contains('\0') {
        return Err("非法路径".to_string());
    }
    let files = file_utils::list_files_flat(&path, file_utils::get_supported_extensions_ref());
    Ok(files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

// ---------------------------------------------------------------------------
// Batch task commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn batch_enqueue(
    app: tauri::AppHandle,
    source_path: String,
    output_path: String,
) -> Result<String, String> {
    let state = get_state(&app)?;
    let id = state
        .batch_queue
        .enqueue(app.clone(), source_path, output_path)
        .await?;
    // 封堵 enqueue 与 worker 退出的竞态：worker 在队列看起来为空的瞬间
    // 置 running=false 退出；若本次 enqueue 恰好落在其后且没有随后的
    // batch_start，任务将永远滞留 Pending。
    if !state.batch_queue.is_running() {
        let engine = state.queue_engine();
        state.batch_queue.start(app.clone(), engine).await;
    }
    Ok(id)
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
    // Clamp to a sane range: 0 would make the worker spin forever without ever
    // claiming a task.
    state.batch_queue.set_concurrency(concurrency.clamp(1, 16));
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


/// Directory that holds the runtime log files (created on demand).
pub(crate) fn log_dir() -> std::path::PathBuf {
    std::env::var("APPDATA")
        .map(|p| std::path::PathBuf::from(p).join("OmniMD").join("logs"))
        .unwrap_or_else(|_| std::env::temp_dir().join("omnimd_logs"))
}

const LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;

/// Rotate `path` to `path.1` once it exceeds `LOG_MAX_BYTES`, so a long-running
/// install cannot grow the log without bound.
fn rotate_log(path: &std::path::Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > LOG_MAX_BYTES {
            let backup = path.with_extension("log.1");
            let _ = std::fs::remove_file(&backup);
            let _ = std::fs::rename(path, &backup);
        }
    }
}

/// File writer that rotates inline when the live log exceeds `LOG_MAX_BYTES`.
/// 启动时的一次性 rotate_log 覆盖不到长会话，这里在写入路径上检查并轮转：
/// rename 当前文件后重新打开（句柄会跟随被改名的文件），rename 失败（被
/// 杀毒软件等占用）时退化为截断，保证日志不会无限增长。
#[derive(Clone)]
struct RotatingFileWriter {
    path: std::path::PathBuf,
    file: Arc<Mutex<fs::File>>,
}

impl io::Write for RotatingFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut f = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let over_limit = f
            .metadata()
            .map(|m| m.len() > LOG_MAX_BYTES)
            .unwrap_or(false);
        if over_limit {
            let backup = self.path.with_extension("log.1");
            let _ = std::fs::remove_file(&backup);
            if std::fs::rename(&self.path, &backup).is_ok() {
                if let Ok(nf) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)
                {
                    *f = nf;
                }
            } else {
                // append 模式下后续写入始终落在文件末尾，截断即重置。
                let _ = f.set_len(0);
            }
        }
        f.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.lock().unwrap_or_else(|e| e.into_inner()).flush()
    }
}

/// Initialize a tracing subscriber that writes to a log file under the
/// user-writable AppData/Roaming/OmniMD/logs directory (falling back to stdout
/// if that file cannot be created). This makes runtime diagnostics inspectable
/// instead of being silently discarded in a bundled build.
fn init_logging() {
    use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*};

    let log_dir = log_dir();
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("omnimd.log");
    rotate_log(&log_path);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();

    // BoxMakeWriter unifies the two writer types so both match arms share one
    // concrete type. The closure form satisfies `MakeWriter` (Fn() -> W);
    // the writer is Clone (Arc<Mutex<File>>), so every write call gets a
    // handle to the same rotating file.
    use tracing_subscriber::fmt::writer::BoxMakeWriter;
    let writer: BoxMakeWriter = match file {
        Some(f) => {
            let rotating = RotatingFileWriter {
                path: log_path,
                file: Arc::new(Mutex::new(f)),
            };
            BoxMakeWriter::new(move || rotating.clone())
        }
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
    let log_dir = log_dir();
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
        // Must be registered first: a second launch focuses the existing
        // window instead of running a second process over the same database.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            // Forward file-path argv from the shell context menu to the frontend.
            if !cli_args.is_empty() {
                let _ = app.emit("argv-files", &cli_args);
            }
            // Mark stale Processing tasks from a previous session as Failed.
            if let Err(e) = db::reconcile_stale_batch_tasks(app.handle()) {
                tracing::warn!("清理上次会话遗留任务状态失败: {e}");
            }
            Ok(())
        })
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            convert_file,
            cancel_task,
            get_default_output_dir,
            open_folder,
            get_supported_formats,
            get_converter_info,
            get_app_version,
            write_text_file,
            read_text_file,
            list_files_in_folder,
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
            cache::get_cache_info,
            cache::clear_webview_cache,
            cache::clear_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
