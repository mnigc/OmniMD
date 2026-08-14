pub mod models;
pub mod converters;
pub mod file_utils;
pub mod pipeline;

use std::sync::Mutex;
use std::collections::HashMap;

use models::converter::Converter;
use models::task::{ConversionError, ConversionResult, ConversionTask, TaskStatus};
use models::ConversionStage;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResultDto {
    pub task_id: String,
    pub markdown: String,
    pub document_serialized: String,
    pub asset_count: usize,
    pub errors: Vec<ErrorDto>,
    pub success: bool,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusDto {
    pub task_id: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResultDto {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub results: Vec<ConversionResultDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConverterInfo {
    pub name: String,
    pub supported_formats: Vec<String>,
}

struct AppState {
    // std::sync::Mutex is intentional here: the lock is held only for
    // short, synchronous inserts/lookups, so it does not block the tokio
    // runtime for any meaningful amount of time.
    tasks: Mutex<HashMap<String, ConversionTask>>,
}

fn result_to_dto(result: &ConversionResult) -> ConversionResultDto {
    ConversionResultDto {
        task_id: result.task_id.clone(),
        markdown: result.markdown.clone(),
        document_serialized: serde_json::to_string(&result.document).unwrap_or_default(),
        asset_count: result.assets.len(),
        errors: result.errors.iter().map(error_to_dto).collect(),
        success: result.errors.is_empty(),
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
) -> Result<ConversionResultDto, String> {
    info!("convert_file: {} -> {}", source_path, output_dir);

    let output_path = file_utils::get_output_path(&source_path, &output_dir);
    let mut task = ConversionTask::new(&source_path, &output_path);
    task.status = TaskStatus::Processing;
    task.stage = ConversionStage::DetectingFormat;
    task.progress = 0.1;

    let state = get_state(&app)?;
    state.tasks.lock().unwrap().insert(task.id.clone(), task.clone());
    emit_progress(&app, &task);

    task.stage = ConversionStage::Extracting;
    task.progress = 0.4;
    emit_progress(&app, &task);

    let result = match pipeline::convert_file(&task).await {
        Ok(r) => r,
        Err(e) => {
            task.status = TaskStatus::Failed;
            task.error = Some(e.message.clone());
            emit_status(&app, &task);
            return Err(format!("[{:?}]: {}", e.code, e.message));
        }
    };

    task.status = TaskStatus::Completed;
    task.progress = 1.0;
    emit_progress(&app, &task);
    emit_status(&app, &task);

    Ok(result_to_dto(&result))
}

#[tauri::command]
async fn convert_batch(
    app: tauri::AppHandle,
    source_paths: Vec<String>,
    output_dir: String,
    concurrency: usize,
) -> Result<BatchResultDto, String> {
    info!(
        "convert_batch: {} files -> {} (concurrency={})",
        source_paths.len(),
        output_dir,
        concurrency
    );

    let mut tasks = Vec::new();
    for path in &source_paths {
        let output_path = file_utils::get_output_path(path, &output_dir);
        let mut task = ConversionTask::new(path, &output_path);
        task.status = TaskStatus::Processing;
        tasks.push(task);
    }

    let state = get_state(&app)?;
    for task in &tasks {
        state.tasks.lock().unwrap().insert(task.id.clone(), task.clone());
        emit_status(&app, task);
    }

    let app_handle = app.clone();
    let results = pipeline::convert_batch(tasks, concurrency.max(1), move |result| {
        let success = result.errors.is_empty();
        let status = if success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        let error_msg = result.errors.first().map(|e| e.message.clone());

        if let Some(state) = get_state(&app_handle).ok() {
            let mut guard = state.tasks.lock().unwrap();
            if let Some(task) = guard.get_mut(&result.task_id) {
                task.progress = 1.0;
                task.stage = ConversionStage::Saving;
                task.status = status.clone();
                task.error = error_msg.clone();
            }
        }

        let _ = app_handle.emit(
            "task-progress",
            TaskProgressDto {
                task_id: result.task_id.clone(),
                progress: 1.0,
                stage: "Saving".to_string(),
            },
        );
        let _ = app_handle.emit(
            "task-status",
            TaskStatusDto {
                task_id: result.task_id.clone(),
                status: format!("{:?}", status),
                error: error_msg,
            },
        );
    })
    .await;

    let dtos: Vec<ConversionResultDto> = results.iter().map(result_to_dto).collect();
    let completed = dtos.iter().filter(|r| r.success).count();
    let failed = dtos.len() - completed;

    Ok(BatchResultDto {
        total: dtos.len(),
        completed,
        failed,
        results: dtos,
    })
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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            tasks: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            convert_file,
            convert_batch,
            get_supported_formats,
            get_converter_info,
            preview_markdown,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
