use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::sync::Mutex;

use crate::engine::DocumentEngine;
use crate::models::task::{
    BatchSummaryDto, Cancellation, ConversionError, ConversionStage, ConversionTask, ErrorCode,
    ProgressCallback, TaskStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgressEvent {
    pub task_id: String,
    pub progress: f32,
    pub stage: String,
    pub elapsed_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
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

fn insert_batch(app: &tauri::AppHandle, id: &str, source: &str, output: &str, now: u64) {
    match crate::db::db(app) {
        Ok(db) => {
            if let Err(e) = db.insert_batch_task(id, source, output, now) {
                tracing::warn!("批量任务落库失败（任务记录将丢失）: {e}");
            }
        }
        Err(e) => tracing::warn!("打开数据库失败，无法落库批量任务: {e}"),
    }
}

fn update_status(app: &tauri::AppHandle, id: &str, status: &str, error: Option<&str>, elapsed: u64) {
    match crate::db::db(app) {
        Ok(db) => {
            if let Err(e) = db.update_batch_task_status(id, status, error, elapsed) {
                tracing::warn!("批量任务状态更新失败（{id} -> {status}）: {e}");
            }
        }
        Err(e) => tracing::warn!("打开数据库失败，无法更新任务状态: {e}"),
    }
}

fn get_pending(app: &tauri::AppHandle, limit: u64) -> Vec<crate::models::task::BatchTaskDto> {
    match crate::db::db(app) {
        Ok(db) => db.list_batch_tasks("Pending", limit, 0).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Removes a task from the in-flight set when its conversion thread exits,
/// including on panic. Without this, a panic inside the engine would leak the
/// concurrency slot forever and eventually stall the whole queue.
struct ActiveTaskGuard {
    task_id: String,
    cancellation: Arc<Cancellation>,
    active_tasks: Arc<Mutex<HashMap<String, Arc<Cancellation>>>>,
    rt: tokio::runtime::Handle,
}

impl Drop for ActiveTaskGuard {
    fn drop(&mut self) {
        let task_id = self.task_id.clone();
        let cancellation = self.cancellation.clone();
        let active_tasks = self.active_tasks.clone();
        // Runs on the dedicated std thread (not inside the async runtime), so
        // `block_on` here is safe and cannot re-enter the runtime.
        self.rt.block_on(async move {
            let mut guard = active_tasks.lock().await;
            // Only free the slot if it still belongs to THIS run: a paused task
            // that was resumed may have installed a fresh cancellation handle,
            // which must not be removed by the stale thread.
            if let Some(current) = guard.get(&task_id) {
                if Arc::ptr_eq(current, &cancellation) {
                    guard.remove(&task_id);
                }
            }
        });
    }
}

pub struct BatchQueue {
    running: Arc<AtomicBool>,
    concurrency: Arc<AtomicU32>,
    active_tasks: Arc<Mutex<HashMap<String, Arc<Cancellation>>>>,
    paused_tasks: Arc<Mutex<HashSet<String>>>,
    /// Per-task run token, bumped whenever a task is (re)claimed or resumed.
    /// A finishing thread only writes a terminal status if its token is still
    /// current, so a stale thread cannot clobber a newer run's state.
    generations: Arc<Mutex<HashMap<String, u64>>>,
    worker_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl BatchQueue {
    pub fn new(concurrency: u32) -> Self {
        BatchQueue {
            running: Arc::new(AtomicBool::new(false)),
            concurrency: Arc::new(AtomicU32::new(concurrency)),
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
            paused_tasks: Arc::new(Mutex::new(HashSet::new())),
            generations: Arc::new(Mutex::new(HashMap::new())),
            worker_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Bump and return the run token for `task_id`.
    async fn next_generation(&self, task_id: &str) -> u64 {
        let mut g = self.generations.lock().await;
        let e = g.entry(task_id.to_string()).or_insert(0);
        *e = e.wrapping_add(1);
        *e
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

    pub async fn enqueue(&self, app: tauri::AppHandle, source_path: String, output_path: String) -> Result<String, String> {
        // Deduplicate: if an active task for the same source already exists,
        // reuse it instead of creating another identical one. This prevents a
        // single dropped file from producing a pile of duplicate tasks.
        //
        // The DB handle MUST be released before `emit_summary` runs: the global
        // DB mutex is a non-reentrant `std::sync::Mutex`, and `emit_summary`
        // re-acquires it. Holding the guard here would self-deadlock the thread
        // (and then freeze every other DB command, i.e. the whole UI).
        let existing_id = match crate::db::db(&app) {
            Ok(db) => db.find_active_batch_task_by_source(&source_path).ok().flatten(),
            Err(_) => None,
        };
        if let Some(existing_id) = existing_id {
            emit_summary(&app);
            return Ok(existing_id);
        }

        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
        let id = uuid::Uuid::new_v4().to_string();

        insert_batch(&app, &id, &source_path, &output_path, now);
        emit_summary(&app);

        Ok(id)
    }

    pub async fn start(&self, app: tauri::AppHandle, engine: Arc<dyn DocumentEngine>) {
        if self.running.swap(true, Ordering::Relaxed) {
            return;
        }

        // Acquire the tokio runtime handle on the caller's runtime thread so the
        // OS worker thread can drive async work through `block_on`.
        let rt = tokio::runtime::Handle::current();

        let engine = engine.clone();
        let active_tasks = self.active_tasks.clone();
        let running = self.running.clone();
        let concurrency = self.concurrency.clone();
        let paused_tasks = self.paused_tasks.clone();
        let generations = self.generations.clone();

        let worker = move || {
            rt.block_on(async {
                loop {
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }

let active = active_tasks.lock().await.len();
                    let concurrency = concurrency.load(Ordering::Relaxed) as usize;

                    if active >= concurrency {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        continue;
                    }

                    // Only fetch as many pending rows as there is room for, so the
                    // in-flight count never exceeds the configured concurrency.
                    // `saturating_sub` guards against a concurrent lowering of the
                    // concurrency limit between the snapshot and this subtraction.
                    let room = concurrency.saturating_sub(active).max(1) as u64;
                    let pending = get_pending(&app, room);

                    if pending.is_empty() {
                        // Exit only when there is nothing pending AND nothing in
                        // flight. Paused tasks are not in `active_tasks`, so they
                        // no longer block the worker from exiting.
                        if active == 0 {
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        continue;
                    }

                    for task_dto in pending {
                        if !running.load(Ordering::Relaxed) {
                            break;
                        }
                        if active_tasks.lock().await.len() >= concurrency {
                            break;
                        }

                        // Claim the task synchronously (mark Processing in the DB
                        // AND register its cancellation in `active_tasks`) BEFORE
                        // spawning, so a subsequent `get_pending` can never return
                        // this task again and spawn a duplicate conversion thread.
                        let tid = task_dto.id.clone();
                        update_status(&app, &tid, "Processing", None, 0);
                        let cancellation = Arc::new(Cancellation::new());
                        active_tasks.lock().await.insert(tid.clone(), cancellation.clone());
                        // Token for this run; a stale thread whose token no
                        // longer matches will not overwrite terminal state.
                        let generation = {
                            let mut g = generations.lock().await;
                            let e = g.entry(tid.clone()).or_insert(0);
                            *e = e.wrapping_add(1);
                            *e
                        };
                        let _ = app.emit("batch-status", BatchStatusEvent {
                            task_id: tid.clone(),
                            status: "Processing".to_string(),
                            error: None,
                            elapsed_secs: 0,
                        });

                        let engine = engine.clone();
                        let app = app.clone();
                        let active = active_tasks.clone();
                        let paused_tasks = paused_tasks.clone();
                        let generations = generations.clone();
                        let rt = rt.clone();

                        std::thread::spawn(move || {
                            let task_id = tid.clone();
                            let task_created_at = task_dto.created_at;
                            let app_for_cb = app.clone();
                            let progress_cb: ProgressCallback = Arc::new(move |p: f32, detail: Option<String>| {
                                let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
                                let elapsed = now.saturating_sub(task_created_at);
                                let _ = app_for_cb.emit("batch-progress", BatchProgressEvent {
                                    task_id: task_id.clone(),
                                    progress: p,
                                    stage: if p < 0.35 { "ModelLoading".to_string() } else if p < 0.9 { "Parsing".to_string() } else { "Saving".to_string() },
                                    elapsed_secs: elapsed,
                                    detail,
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
                            };

                            let created_at = task_dto.created_at;

                            // Frees the concurrency slot on scope exit even if the
                            // engine panics (see `ActiveTaskGuard`).
                            let _active_guard = ActiveTaskGuard {
                                task_id: tid.clone(),
                                cancellation: cancellation.clone(),
                                active_tasks: active.clone(),
                                rt: rt.clone(),
                            };

                            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                rt.block_on(engine.convert(&conv_task, Some(progress_cb), Some(&cancellation)))
                            }))
                            .unwrap_or_else(|_| {
                                Err(ConversionError {
                                    code: ErrorCode::EngineError,
                                    message: "转换线程异常退出（引擎内部错误）".to_string(),
                                    stage: ConversionStage::Parsing,
                                    retryable: true,
                                    page: None,
                                })
                            });

                            let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
                            let elapsed = now.saturating_sub(created_at);

                            // A stale run (its task was resumed and re-claimed)
                            // must not clobber the newer run's state.
                            let is_current = rt.block_on(async {
                                generations.lock().await.get(&tid).copied() == Some(generation)
                            });

                            if is_current {
                                match &result {
                                    Ok(_) => {
                                        update_status(&app, &tid, "Completed", None, elapsed);
                                        let _ = app.emit("batch-status", BatchStatusEvent {
                                            task_id: tid.clone(), status: "Completed".to_string(), error: None, elapsed_secs: elapsed,
                                        });
                                    }
                                    Err(e) => {
                                        let status = if cancellation.cancelled() {
                                            // If the task was paused (cooperative
                                            // cancel + DB already set to Paused),
                                            // keep it Paused instead of overwriting
                                            // with Cancelled.
                                            let is_paused = rt.block_on(async {
                                                paused_tasks.lock().await.remove(&tid)
                                            });
                                            if is_paused { "Paused" } else { "Cancelled" }
                                        } else {
                                            "Failed"
                                        };
                                        update_status(&app, &tid, status, Some(&e.message), elapsed);
                                        let _ = app.emit("batch-status", BatchStatusEvent {
                                            task_id: tid.clone(), status: status.to_string(), error: Some(e.message.clone()), elapsed_secs: elapsed,
                                        });
                                    }
                                }
                            }
                            emit_summary(&app);
                        });
                    }

                    // Small backoff so the loop never busy-spins even if the DB
                    // writes from the spawned threads lag behind the next fetch.
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
});
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

        // Mark locally so the worker can distinguish a pause from a real cancel
        // when `convert` returns the cooperative-cancel error.
        self.paused_tasks.lock().await.insert(task_id.to_string());

        update_status(app, task_id, "Paused", None, 0);
        let _ = app.emit("batch-status", BatchStatusEvent {
            task_id: task_id.to_string(), status: "Paused".to_string(), error: None, elapsed_secs: 0,
        });
        Ok(())
    }

    pub async fn resume_task(
        &self,
        app: &tauri::AppHandle,
        engine: Arc<dyn DocumentEngine>,
        task_id: &str,
    ) -> Result<(), String> {
        self.paused_tasks.lock().await.remove(task_id);
        // Invalidate any still-running thread for this task so it cannot write
        // a terminal status over the fresh run we are about to enqueue.
        self.next_generation(task_id).await;

        update_status(app, task_id, "Pending", None, 0);
        let _ = app.emit("batch-status", BatchStatusEvent {
            task_id: task_id.to_string(), status: "Pending".to_string(), error: None, elapsed_secs: 0,
        });
        if !self.is_running() {
            self.start(app.clone(), engine).await;
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

        // Collect the targets while borrowing the DB, then RELEASE the DB
        // handle before updating: `update_status` acquires the same global DB
        // mutex, and std Mutex is not reentrant — updating inside the borrow
        // deadlocks the calling thread instantly, and the forever-held mutex
        // then freezes every later DB command (including main-thread sync
        // commands, i.e. the whole UI).
        let mut targets: Vec<(String, u64)> = Vec::new();
        if let Ok(db) = crate::db::db(app) {
            for status in &["Processing", "Pending"] {
                // Paginate through all tasks of this status (not just 100).
                let mut offset = 0u64;
                const PAGE: u64 = 200;
                loop {
                    let batch = db.list_batch_tasks(status, PAGE, offset).unwrap_or_default();
                    if batch.is_empty() {
                        break;
                    }
                    for t in &batch {
                        let elapsed = if *status == "Processing" {
                            now.saturating_sub(t.created_at)
                        } else {
                            0
                        };
                        targets.push((t.id.clone(), elapsed));
                    }
                    if batch.len() < PAGE as usize {
                        break;
                    }
                    offset += PAGE;
                }
            }
        }
        for (id, elapsed) in &targets {
            update_status(app, id, "Cancelled", None, *elapsed);
        }
        emit_summary(app);
        Ok(())
    }

    pub async fn retry_task(&self, app: &tauri::AppHandle, engine: Arc<dyn DocumentEngine>, task_id: &str) -> Result<(), String> {
        update_status(app, task_id, "Pending", None, 0);
        if !self.is_running() {
            self.start(app.clone(), engine).await;
        }
        emit_summary(app);
        Ok(())
    }

    pub async fn retry_failed(&self, app: &tauri::AppHandle, engine: Arc<dyn DocumentEngine>) -> Result<(), String> {
        // Collect the failed task IDs while borrowing the DB, then RELEASE the
        // DB handle before updating: `update_status` acquires the same global
        // DB mutex, and std Mutex is not reentrant — updating inside the
        // borrow deadlocks the calling thread instantly (self-reentrant lock),
        // permanently freezing every later DB command including main-thread
        // sync commands.
        let mut ids: Vec<String> = Vec::new();
        if let Ok(db) = crate::db::db(app) {
            let mut offset = 0u64;
            const PAGE: u64 = 200;
            loop {
                let batch = db.list_batch_tasks("Failed", PAGE, offset).unwrap_or_default();
                if batch.is_empty() {
                    break;
                }
                for t in &batch {
                    ids.push(t.id.clone());
                }
                if batch.len() < PAGE as usize {
                    break;
                }
                offset += PAGE;
            }
        }
        for id in &ids {
            update_status(app, id, "Pending", None, 0);
        }
        let any = !ids.is_empty();
        if any && !self.is_running() {
            self.start(app.clone(), engine).await;
        }
        emit_summary(app);
        Ok(())
    }

    pub async fn clear_done(&self, app: &tauri::AppHandle) -> Result<(), String> {
        // "清空列表" is expected to clear the whole list. Worker-claimed tasks
        // live in `active_tasks`; cancel them first so their threads exit
        // cooperatively instead of writing status back to already-deleted rows
        // (a harmless but wasteful no-op write). The UI disables this action
        // while conversions are in flight, so normally this set is empty except
        // for the brief claim window race.
        let mut active = self.active_tasks.lock().await;
        for (_, cancellation) in active.drain() {
            cancellation.cancel();
        }
        drop(active);

        if let Ok(db) = crate::db::db(app) {
            for status in ["Pending", "Completed", "Cancelled", "Failed"] {
                if let Err(e) = db.delete_batch_tasks(status) {
                    tracing::warn!("清理批量任务记录失败（{status}）: {e}");
                }
            }
        }
        emit_summary(app);
        Ok(())
    }
}