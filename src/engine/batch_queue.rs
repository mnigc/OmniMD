use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::sync::Mutex;

use crate::engine::mineru_engine::MinerUEngine;
use crate::engine::DocumentEngine;
use crate::models::ocr::{Cancellation, ProgressCallback};
use crate::models::task::{BatchSummaryDto, ConversionStage, ConversionTask, OutputMode, ParseQuality, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgressEvent {
    pub task_id: String,
    pub progress: f32,
    pub stage: String,
    pub elapsed_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchStatusEvent {
    pub task_id: String,
    pub status: String,
    pub error: Option<String>,
    pub elapsed_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSummaryEvent {
    pub summary: BatchSummaryDto,
}

fn emit_summary(app: &tauri::AppHandle) {
    if let Ok(db) = crate::db::db(app) {
        if let Ok(summary) = db.get_batch_summary() {
            let _ = app.emit("batch-summary", BatchSummaryEvent { summary });
        }
    }
}

fn insert_batch(app: &tauri::AppHandle, id: &str, source: &str, output: &str, mode: &OutputMode, quality: &ParseQuality, now: u64) {
    if let Ok(db) = crate::db::db(app) {
        let _ = db.insert_batch_task(id, source, output, mode, quality, now);
    }
}

fn update_status(app: &tauri::AppHandle, id: &str, status: &str, error: Option<&str>, elapsed: u64) {
    if let Ok(db) = crate::db::db(app) {
        let _ = db.update_batch_task_status(id, status, error, elapsed);
    }
}

fn get_pending(app: &tauri::AppHandle, limit: u64) -> Vec<crate::models::task::BatchTaskDto> {
    match crate::db::db(app) {
        Ok(db) => db.list_batch_tasks("Pending", limit, 0).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn has_active_tasks(app: &tauri::AppHandle) -> bool {
    match crate::db::db(app) {
        Ok(db) => db.get_batch_summary().ok().map(|s| s.pending + s.processing + s.paused > 0).unwrap_or(false),
        Err(_) => false,
    }
}

pub struct BatchQueue {
    engine: tokio::sync::Mutex<Option<Arc<MinerUEngine>>>,
    running: Arc<AtomicBool>,
    concurrency: Arc<AtomicU32>,
    active_tasks: Arc<Mutex<HashMap<String, Arc<Cancellation>>>>,
    worker_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl BatchQueue {
    pub fn new(concurrency: u32) -> Self {
        BatchQueue {
            engine: tokio::sync::Mutex::new(None),
            running: Arc::new(AtomicBool::new(false)),
            concurrency: Arc::new(AtomicU32::new(concurrency)),
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
            worker_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_engine(&self, engine: Arc<MinerUEngine>) {
        *self.engine.lock().await = Some(engine);
    }

    pub fn set_concurrency(&self, n: u32) {
        self.concurrency.store(n, Ordering::Relaxed);
    }

    pub fn concurrency(&self) -> u32 {
        self.concurrency.load(Ordering::Relaxed)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub async fn enqueue(&self, app: tauri::AppHandle, source_path: String, output_path: String, output_mode: OutputMode, parse_quality: ParseQuality) -> Result<String, String> {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
        let id = uuid::Uuid::new_v4().to_string();

        insert_batch(&app, &id, &source_path, &output_path, &output_mode, &parse_quality, now);
        emit_summary(&app);

        Ok(id)
    }

    pub async fn start(&self, app: tauri::AppHandle) {
        if self.running.swap(true, Ordering::Relaxed) {
            return;
        }

        let guard = self.engine.lock().await;
        let engine = guard.as_ref().expect("BatchQueue engine not set").clone();
        drop(guard);

        let active_tasks = self.active_tasks.clone();
        let running = self.running.clone();
        let _concurrency = self.concurrency.clone();

        let worker = move || {
            let rt = tokio::runtime::Handle::current();
            loop {
                if !running.load(Ordering::Relaxed) {
                    break;
                }

                let pending = get_pending(&app, 10);
                if pending.is_empty() {
                    if !has_active_tasks(&app) {
                        running.store(false, Ordering::Relaxed);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }

                for task_dto in pending {
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }

                    let engine = engine.clone();
                    let app = app.clone();
                    let active = active_tasks.clone();
                    let rt = rt.clone();

                    std::thread::spawn(move || {
                        update_status(&app, &task_dto.id, "Processing", None, 0);

                        let _ = app.emit("batch-status", BatchStatusEvent {
                            task_id: task_dto.id.clone(),
                            status: "Processing".to_string(),
                            error: None,
                            elapsed_secs: 0,
                        });

                        let cancellation = Arc::new(Cancellation::new());
                        rt.block_on(async {
                            active.lock().await.insert(task_dto.id.clone(), cancellation.clone());
                        });

                        let task_id = task_dto.id.clone();
                        let app_for_cb = app.clone();
                        let progress_cb: ProgressCallback = Arc::new(move |p: f32, _detail: Option<String>| {
                            let elapsed = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
                            let _ = app_for_cb.emit("batch-progress", BatchProgressEvent {
                                task_id: task_id.clone(),
                                progress: p,
                                stage: if p < 0.35 { "ModelLoading".to_string() } else if p < 0.9 { "Parsing".to_string() } else { "Saving".to_string() },
                                elapsed_secs: elapsed,
                            });
                        });

                        let conv_task = ConversionTask {
                            id: task_dto.id.clone(),
                            source_path: task_dto.source_path.clone(),
                            output_path: task_dto.output_path.clone(),
                            status: TaskStatus::Processing,
                            progress: 0.0,
                            stage: ConversionStage::Queued,
                            error: None,
                            created_at: task_dto.created_at,
                            completed_at: None,
                            output_mode: task_dto.output_mode.clone(),
                            ai_ready_opts: crate::models::task::AiReadyOpts::default(),
                            parse_quality: task_dto.parse_quality.clone(),
                        };

                        let tid = task_dto.id.clone();
                        let created_at = task_dto.created_at;

                        let result = rt.block_on(engine.convert(&conv_task, Some(progress_cb), Some(&cancellation)));

                        rt.block_on(async {
                            active.lock().await.remove(&tid);
                        });

                        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
                        let elapsed = now.saturating_sub(created_at);

                        match &result {
                            Ok(_) => {
                                update_status(&app, &tid, "Completed", None, elapsed);
                                let _ = app.emit("batch-status", BatchStatusEvent {
                                    task_id: tid, status: "Completed".to_string(), error: None, elapsed_secs: elapsed,
                                });
                            }
                            Err(e) => {
                                let status = if cancellation.cancelled() { "Cancelled" } else { "Failed" };
                                update_status(&app, &tid, status, Some(&e.message), elapsed);
                                let _ = app.emit("batch-status", BatchStatusEvent {
                                    task_id: tid, status: status.to_string(), error: Some(e.message.clone()), elapsed_secs: elapsed,
                                });
                            }
                        }
                        emit_summary(&app);
                    });
                }
            }
        };

        let handle = std::thread::spawn(worker);
        *self.worker_handle.lock().await = Some(tokio::task::spawn_blocking(move || {
            let _ = handle.join();
        }));
    }

    pub async fn pause_task(&self, app: &tauri::AppHandle, task_id: &str) -> Result<(), String> {
        let mut active = self.active_tasks.lock().await;
        if let Some(cancellation) = active.remove(task_id) {
            cancellation.cancel();
        }
        drop(active);

        update_status(app, task_id, "Paused", None, 0);
        let _ = app.emit("batch-status", BatchStatusEvent {
            task_id: task_id.to_string(), status: "Paused".to_string(), error: None, elapsed_secs: 0,
        });
        Ok(())
    }

    pub async fn resume_task(&self, app: &tauri::AppHandle, task_id: &str) -> Result<(), String> {
        update_status(app, task_id, "Pending", None, 0);
        let _ = app.emit("batch-status", BatchStatusEvent {
            task_id: task_id.to_string(), status: "Pending".to_string(), error: None, elapsed_secs: 0,
        });
        if !self.is_running() {
            self.start(app.clone()).await;
        }
        Ok(())
    }

    pub async fn cancel_task(&self, app: &tauri::AppHandle, task_id: &str) -> Result<(), String> {
        let mut active = self.active_tasks.lock().await;
        if let Some(cancellation) = active.remove(task_id) {
            cancellation.cancel();
        }
        drop(active);

        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
        let created_at = match crate::db::db(app) {
            Ok(db) => db.get_batch_task_created_at(task_id).unwrap_or(None).unwrap_or(now),
            Err(_) => now,
        };
        let elapsed = now.saturating_sub(created_at);

        update_status(app, task_id, "Cancelled", None, elapsed);
        let _ = app.emit("batch-status", BatchStatusEvent {
            task_id: task_id.to_string(), status: "Cancelled".to_string(), error: None, elapsed_secs: elapsed,
        });
        Ok(())
    }

    pub async fn cancel_all(&self, app: &tauri::AppHandle) -> Result<(), String> {
        let mut active = self.active_tasks.lock().await;
        for (_, cancellation) in active.drain() {
            cancellation.cancel();
        }
        drop(active);

        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();

        if let Ok(db) = crate::db::db(app) {
            for t in db.list_batch_tasks("Processing", 100, 0).unwrap_or_default() {
                let elapsed = now.saturating_sub(t.created_at);
                update_status(app, &t.id, "Cancelled", None, elapsed);
            }
            for t in db.list_batch_tasks("Pending", 100, 0).unwrap_or_default() {
                update_status(app, &t.id, "Cancelled", None, 0);
            }
        }
        emit_summary(app);
        Ok(())
    }

    pub async fn retry_failed(&self, app: &tauri::AppHandle) -> Result<(), String> {
        let mut any = false;
        if let Ok(db) = crate::db::db(app) {
            for t in db.list_batch_tasks("Failed", 100, 0).unwrap_or_default() {
                update_status(app, &t.id, "Pending", None, 0);
                any = true;
            }
        }
        if any && !self.is_running() {
            self.start(app.clone()).await;
        }
        emit_summary(app);
        Ok(())
    }

    pub async fn clear_done(&self, app: &tauri::AppHandle) -> Result<(), String> {
        if let Ok(db) = crate::db::db(app) {
            let _ = db.delete_batch_tasks("Completed");
            let _ = db.delete_batch_tasks("Cancelled");
        }
        emit_summary(app);
        Ok(())
    }
}