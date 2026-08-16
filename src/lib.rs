pub mod models;
pub mod converters;
pub mod file_utils;
pub mod pipeline;
pub mod markdown_pipeline;
pub mod web_extractor;
pub mod ocr;

use std::sync::Mutex;
use std::sync::Arc;
use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use models::converter::Converter;
use models::ocr::{Cancellation, OcrMode, ProgressCallback};
use models::task::{
    AiReadyOpts, ConversionError, ConversionResult, ConversionTask, ErrorCode, TaskStatus,
    OutputMode,
};
use models::ConversionStage;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversionStatsDto {
    pub image_count: usize,
    pub table_count: usize,
    pub word_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr_page_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr_char_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_confidence_permille: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_confidence_count: Option<usize>,
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
pub struct TaskProgressDto {
    pub task_id: String,
    pub progress: f32,
    pub stage: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            tasks: Mutex::new(HashMap::new()),
            cancellations: Mutex::new(HashMap::new()),
        }
    }
}

fn result_to_dto(result: &ConversionResult) -> ConversionResultDto {
    let stats = match &result.stats {
        Some(s) => ConversionStatsDto {
            image_count: s.image_count,
            table_count: s.table_count,
            word_count: s.word_count,
            ocr_page_count: s.ocr_page_count,
            ocr_char_count: s.ocr_char_count,
            avg_confidence_permille: s.avg_confidence_permille,
            low_confidence_count: s.low_confidence_count,
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
    ocr_mode: Option<OcrMode>,
    client_task_id: Option<String>,
) -> Result<ConversionResultDto, String> {
    info!("convert_file: {} -> {} (mode={:?}, ocr={:?})", source_path, output_dir, output_mode, ocr_mode);

    let mode = output_mode
        .as_deref()
        .map(OutputMode::from_str)
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
    task.ocr_mode = ocr_mode.unwrap_or_default();
    task.status = TaskStatus::Processing;
    task.stage = ConversionStage::DetectingFormat;
    task.progress = 0.1;

    let state = get_state(&app)?;
    let cancellation = Cancellation::new();
    state.cancellations.lock().unwrap().insert(task.id.clone(), cancellation.clone());
    state.tasks.lock().unwrap().insert(task.id.clone(), task.clone());
    emit_progress(&app, &task);

    task.stage = ConversionStage::Extracting;
    task.progress = 0.4;
    emit_progress(&app, &task);

    // Create a progress callback that emits events to the frontend.
    // The callback receives values in [0, 1] and maps them to the [0.4, 0.95] range.
    let app_clone = app.clone();
    let task_id = task.id.clone();
    let progress_cb: ProgressCallback = Arc::new(move |p: f32, detail: Option<String>| {
        let stage = if p < 0.05 {
            "Extracting".to_string()
        } else {
            "Ocr".to_string()
        };
        let progress = 0.4 + p * 0.55; // map [0,1] → [0.4, 0.95]
        let _ = app_clone.emit(
            "task-progress",
            TaskProgressDto {
                task_id: task_id.clone(),
                progress,
                stage,
                detail,
            },
        );
    });

    let result = match pipeline::convert_file(&task, Some(progress_cb), Some(&cancellation)).await {
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
    state.cancellations.lock().unwrap().remove(task_id);
}

/// Remove a completed/failed/cancelled task from the tasks map to prevent
/// unbounded memory growth. The frontend receives the result directly via the
/// command return value, so the map entry is no longer needed after the task
/// reaches a terminal state.
fn cleanup_task(state: &tauri::State<'_, AppState>, task_id: &str) {
    state.tasks.lock().unwrap().remove(task_id);
}

/// Request cancellation of a running conversion task. The backend cooperatively
/// stops at the next checkpoint (before writing files, between OCR pages, etc.).
#[tauri::command]
fn cancel_task(app: tauri::AppHandle, task_id: String) -> Result<(), String> {
    let state = get_state(&app)?;
    if let Some(cancellation) = state.cancellations.lock().unwrap().get(&task_id) {
        cancellation.cancel();
        tracing::info!("Cancel requested for task {}", task_id);
    }
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

    if cancellation.cancelled() {
        cleanup_cancellation(&state, &task.id);
    cleanup_task(&state, &task.id);
        return Err("cancelled".to_string());
    }

    task.stage = ConversionStage::Fetching;
    task.progress = 0.3;
    emit_progress(&app, &task);

    let html = web_extractor::fetch_html(&url).await?;

    if cancellation.cancelled() {
        task.status = TaskStatus::Cancelled;
        task.error = Some("任务已取消".to_string());
        emit_status(&app, &task);
        cleanup_cancellation(&state, &task.id);
    cleanup_task(&state, &task.id);
        return Err("cancelled".to_string());
    }

    task.stage = ConversionStage::Extracting;
    task.progress = 0.5;
    emit_progress(&app, &task);

    let extracted = web_extractor::extract_content(&html, &url)
        .map_err(|e| format!("Failed to extract content: {}", e))?;

    task.stage = ConversionStage::Structuring;
    task.progress = 0.7;
    emit_progress(&app, &task);

    if cancellation.cancelled() {
        task.status = TaskStatus::Cancelled;
        task.error = Some("任务已取消".to_string());
        emit_status(&app, &task);
        cleanup_cancellation(&state, &task.id);
    cleanup_task(&state, &task.id);
        return Err("cancelled".to_string());
    }

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

    if cancellation.cancelled() {
        task.status = TaskStatus::Cancelled;
        task.error = Some("任务已取消".to_string());
        emit_status(&app, &task);
        cleanup_cancellation(&state, &task.id);
    cleanup_task(&state, &task.id);
        return Err("cancelled".to_string());
    }

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
            ocr_page_count: None,
            ocr_char_count: None,
            avg_confidence_permille: None,
            low_confidence_count: None,
        },
    };

    Ok(result)
}

/// Return the default output directory: the directory that contains the
/// running binary, with an `output/` subdirectory appended. This mirrors
/// "installed app folder / output" so that users who never touch the
/// settings page still get a sensible, discoverable location.
#[tauri::command]
fn get_default_output_dir() -> String {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let install_dir = exe.parent().unwrap_or(exe.as_path());
    let out = install_dir.join("output");
    out.to_string_lossy().replace("\\", "/").to_string()
}

/// Open the given directory in the system file manager (Explorer / Finder /
/// nautilus). Returns early when the path is empty so the frontend can treat
/// this as a no-op.
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
    let converter = converters::get_converter();
    let info = ConverterInfo {
        name: converter.name().to_string(),
        supported_formats: get_supported_formats(),
    };
    serde_json::to_string(&info).unwrap_or_default()
}

#[tauri::command]
fn preview_markdown(markdown: String) -> String {
    markdown
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
        .unwrap_or("downloaded_file")
        .to_string();

    let sanitized: String = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect();
    let sanitized = if sanitized.is_empty() {
        "downloaded_file".to_string()
    } else {
        sanitized
    };

    let file_path = temp_dir.join(&sanitized);
    fs::write(&file_path, &bytes)
        .map_err(|e| format!("保存文件失败: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

pub fn run() {
    // Clean up leftover temp download files from previous runs to prevent
    // disk space accumulation.
    let temp_dir = std::env::temp_dir().join("omnimd_downloads");
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            convert_file,
            fetch_url,
            cancel_task,
            get_default_output_dir,
            open_folder,
            get_supported_formats,
            get_converter_info,
            preview_markdown,
            write_text_file,
            read_text_file,
            list_files_in_folder,
            download_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}