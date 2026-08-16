use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::mineru_runtime::MinerURuntime;
use crate::engine::DocumentEngine;
use crate::models::document::Document;
use crate::models::ocr::{Cancellation, ProgressCallback};
use crate::models::task::{
    ConversionError, ConversionResult, ConversionStage, ConversionTask, ErrorCode,
};

/// MinerU task states (task-level status, no per-page progress).
///
/// M0 (2026-08-16): values are lowercase — `pending` / `queued` /
/// `processing` / `completed` / `failed` / `cancelled`. `queued` is what a
/// new task gets while the server is at its concurrency cap (3); the client
/// must keep polling rather than treat it as an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum MinerTaskStatus {
    Pending,
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MinerTask {
    task_id: String,
    status: MinerTaskStatus,
    #[serde(default)]
    error: Option<String>,
    /// Position in the server-side queue when submitted at concurrency cap.
    #[serde(default)]
    queued_ahead: Option<u64>,
}

/// `GET /tasks/{id}/result` payload (M0 verified).
/// The server returns the parsed output inline as JSON — there is no
/// output-dir path; `results` is keyed by source file stem.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct MinerTaskResult {
    #[serde(default)]
    results: std::collections::HashMap<String, MinerFileResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MinerFileResult {
    #[serde(default)]
    md_content: Option<String>,
    #[serde(default)]
    content_list: Option<serde_json::Value>,
    /// Present when `return_images=true` (M0 verified): maps the filename
    /// referenced in `md_content` as `images/<filename>` to an inline data
    /// URI, e.g. `data:image/jpeg;base64,...`.
    #[serde(default)]
    images: std::collections::HashMap<String, String>,
}

/// Engine backed by the official `mineru-api` FastAPI service.
///
/// The Tauri backend hosts `mineru-api` as a child process (`MinerURuntime`)
/// and drives it over HTTP: `POST /tasks` (async, returns `task_id`),
/// `GET /tasks/{id}` (poll status), `GET /tasks/{id}/result` (fetch output).
///
/// Request field names and status values were verified against MinerU 3.4.5
/// during M0 (see `poc/mineru_poc/`): multipart fields `files`,
/// `parse_method`, `backend`, `formula_enable`, `table_enable`, `return_md`,
/// `return_content_list`.
pub struct MinerUEngine {
    runtime: std::sync::Arc<MinerURuntime>,
    client: reqwest::Client,
}

impl MinerUEngine {
    pub fn new(runtime: std::sync::Arc<MinerURuntime>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .unwrap_or_default();
        MinerUEngine { runtime, client }
    }

    /// Submit a parse task and wait for completion. Emits coarse stage
    /// progress (model loading -> parsing -> post-processing).
    async fn submit_and_wait(
        &self,
        task: &ConversionTask,
        on_progress: Option<ProgressCallback>,
        cancelled: Option<&Cancellation>,
    ) -> Result<MinerTaskResult, ConversionError> {
        self.runtime
            .ensure_running()
            .await
            .map_err(|e| ConversionError {
                code: ErrorCode::RuntimeNotReady,
                message: e,
                stage: ConversionStage::Queued,
                retryable: true,
                page: None,
            })?;

        let file_path = PathBuf::from(&task.source_path);
        let file_name = file_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "input".to_string());

        let file_bytes = std::fs::read(&file_path).map_err(|e| ConversionError {
            code: ErrorCode::IoError,
            message: format!("读取文件失败: {}", e),
            stage: ConversionStage::Queued,
            retryable: false,
            page: None,
        })?;

        // POST /tasks with multipart file + options.
        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("application/octet-stream")
            .map_err(|e| ConversionError {
                code: ErrorCode::EngineError,
                message: format!("准备上传失败: {}", e),
                stage: ConversionStage::Queued,
                retryable: false,
                page: None,
            })?;

        // M0: the multipart field for files is `files` (plural) even for a
        // single file; boolean options are `formula_enable`/`table_enable`.
        let mut form = reqwest::multipart::Form::new().part("files", part);
        form = form.text("parse_method", "auto");
        form = form.text("backend", task.parse_quality.mineru_backend().to_string());
        form = form.text("formula_enable", "true");
        form = form.text("table_enable", "true");
        form = form.text("return_md", "true");
        form = form.text("return_content_list", "true");
        // M0 verified: without this the result omits `images` and the
        // `images/<file>` references in the markdown would be broken.
        form = form.text("return_images", "true");

        let submit_url = format!("{}/tasks", self.runtime.base_url);
        let resp = self
            .client
            .post(&submit_url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| ConversionError {
                code: ErrorCode::EngineError,
                message: format!("提交 MinerU 任务失败: {}", e),
                stage: ConversionStage::ModelLoading,
                retryable: true,
                page: None,
            })?;

        if !resp.status().is_success() {
            let status_code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConversionError {
                code: ErrorCode::EngineError,
                message: format!("MinerU 提交任务失败 ({}): {}", status_code, body),
                stage: ConversionStage::ModelLoading,
                retryable: true,
                page: None,
            });
        }

        let submitted: MinerTask = resp.json().await.map_err(|e| ConversionError {
            code: ErrorCode::EngineError,
            message: format!("解析 MinerU 任务响应失败: {}", e),
            stage: ConversionStage::ModelLoading,
            retryable: false,
            page: None,
        })?;

        let task_id = submitted.task_id;
        if let Some(n) = submitted.queued_ahead.filter(|n| *n > 0) {
            tracing::info!(
                "mineru task queued: app={} mineru={} ahead={}",
                task.id,
                task_id,
                n
            );
        }
        info_task(&task.id, &task_id);

        // Poll GET /tasks/{id} until terminal.
        let poll_url = format!("{}/tasks/{}", self.runtime.base_url, task_id);
        let result_url = format!("{}/tasks/{}/result", self.runtime.base_url, task_id);

        loop {
            if let Some(c) = cancelled {
                if c.cancelled() {
                    return Err(ConversionError {
                        code: ErrorCode::Cancelled,
                        message: "任务已取消".to_string(),
                        stage: ConversionStage::Parsing,
                        retryable: false,
                        page: None,
                    });
                }
            }

            let poll = self.client.get(&poll_url).send().await;
            match poll {
                Ok(resp) if resp.status().is_success() => {
                    let status: MinerTask = resp.json().await.unwrap_or(MinerTask {
                        task_id: task_id.clone(),
                        status: MinerTaskStatus::Unknown,
                        error: None,
                        queued_ahead: None,
                    });
                    match status.status {
                        MinerTaskStatus::Completed => {
                            if let Some(cb) = &on_progress {
                                cb(0.85, Some("生成输出".to_string()));
                            }
                            let result = match self.client.get(&result_url).send().await {
                                Ok(r) => r.json::<MinerTaskResult>().await.unwrap_or_default(),
                                Err(_) => MinerTaskResult::default(),
                            };
                            return Ok(result);
                        }
                        MinerTaskStatus::Failed => {
                            let msg = status
                                .error
                                .clone()
                                .unwrap_or_else(|| "MinerU 解析失败".to_string());
                            return Err(ConversionError {
                                code: ErrorCode::EngineError,
                                message: msg,
                                stage: ConversionStage::Parsing,
                                retryable: true,
                                page: None,
                            });
                        }
                        MinerTaskStatus::Cancelled => {
                            return Err(ConversionError {
                                code: ErrorCode::Cancelled,
                                message: "任务已取消".to_string(),
                                stage: ConversionStage::Parsing,
                                retryable: false,
                                page: None,
                            });
                        }
                        // Queued/Pending: waiting in the server-side queue
                        // (concurrency cap of 3) — keep polling, do not error.
                        MinerTaskStatus::Processing
                        | MinerTaskStatus::Pending
                        | MinerTaskStatus::Queued => {
                            if let Some(cb) = &on_progress {
                                let (p, label) = match status.status {
                                    MinerTaskStatus::Pending | MinerTaskStatus::Queued => {
                                        (0.3, "排队中".to_string())
                                    }
                                    _ => (0.55, "解析中".to_string()),
                                };
                                cb(p, Some(label));
                            }
                            tokio::time::sleep(Duration::from_millis(1000)).await;
                        }
                        MinerTaskStatus::Unknown => {
                            tokio::time::sleep(Duration::from_millis(1000)).await;
                        }
                    }
                }
                _ => {
                    // Server unreachable mid-task: attempt one crash-restart.
                    let _ = self.runtime.restart().await;
                    return Err(ConversionError {
                        code: ErrorCode::EngineError,
                        message: "MinerU 服务连接中断".to_string(),
                        stage: ConversionStage::Parsing,
                        retryable: true,
                        page: None,
                    });
                }
            }
        }
    }

    /// Extract the parsed markdown from the inline JSON result and write it to
    /// `task.output_path`. `results` is keyed by source file stem (M0
    /// verified); fall back to the first non-empty entry if the key is absent.
    fn extract_markdown(
        &self,
        result: &MinerTaskResult,
        task: &ConversionTask,
    ) -> Result<(String, PathBuf), ConversionError> {
        let stem = Path::new(&task.source_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let file = result.results.get(&stem).or_else(|| {
            result
                .results
                .values()
                .find(|r| r.md_content.as_deref().is_some_and(|s| !s.is_empty()))
        });
        let mut md = file
            .and_then(|r| r.md_content.clone())
            .ok_or_else(|| ConversionError {
                code: ErrorCode::EngineError,
                message: "MinerU 结果中缺少 markdown 内容".to_string(),
                stage: ConversionStage::PostProcessing,
                retryable: false,
                page: None,
            })?;

        // M0 verified: with `return_images=true` the API returns inline data
        // URIs keyed by `images/<filename>`. Substitute them so the markdown
        // stays self-contained (no output-dir dependency for images).
        if let Some(imgs) = file.and_then(|r| (!r.images.is_empty()).then(|| &r.images)) {
            for (name, uri) in imgs {
                md = md.replace(&format!("images/{name}"), uri);
            }
        }

        let out_path = PathBuf::from(&task.output_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConversionError {
                code: ErrorCode::IoError,
                message: format!("创建输出目录失败: {}", e),
                stage: ConversionStage::Saving,
                retryable: false,
                page: None,
            })?;
        }
        std::fs::write(&out_path, &md).map_err(|e| ConversionError {
            code: ErrorCode::IoError,
            message: format!("写入 Markdown 失败: {}", e),
            stage: ConversionStage::Saving,
            retryable: false,
            page: None,
        })?;

        Ok((md, out_path))
    }
}

#[async_trait::async_trait]
impl DocumentEngine for MinerUEngine {
    fn name(&self) -> &str {
        "MinerU 3.x"
    }

    fn is_available(&self) -> bool {
        // Synchronous probe is not possible here; treat "we can spawn the
        // process" as availability, and let convert() report readiness.
        true
    }

    async fn convert(
        &self,
        task: &ConversionTask,
        on_progress: Option<ProgressCallback>,
        cancelled: Option<&Cancellation>,
    ) -> Result<ConversionResult, ConversionError> {
        if let Some(cb) = &on_progress {
            cb(0.1, Some("准备文件".to_string()));
        }

        let result = self.submit_and_wait(task, on_progress.clone(), cancelled).await?;

        if let Some(cb) = &on_progress {
            cb(0.9, Some("保存结果".to_string()));
        }

        let (markdown, markdown_path) = self.extract_markdown(&result, task)?;

        let image_count = crate::markdown_pipeline::count_images(&markdown);
        let table_count = crate::markdown_pipeline::count_table_separators(&markdown);
        let word_count = crate::markdown_pipeline::count_words(&markdown);

        let stats = crate::models::task::ConversionStats {
            image_count,
            table_count,
            word_count,
        };

        Ok(ConversionResult {
            task_id: task.id.clone(),
            markdown: markdown.clone(),
            document: Document::new(
                task.source_path
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("document"),
                "markdown",
                std::fs::metadata(&task.source_path)
                    .map(|m| m.len())
                    .unwrap_or(0),
            ),
            assets: Vec::new(),
            errors: Vec::new(),
            output_path: markdown_path.to_string_lossy().to_string(),
            stats: Some(stats),
        })
    }
}

fn info_task(_app_task_id: &str, _miner_task_id: &str) {
    tracing::info!(
        "mineru task submitted: app={} mineru={}",
        _app_task_id,
        _miner_task_id
    );
}
