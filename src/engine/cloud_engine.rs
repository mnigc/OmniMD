use std::error::Error;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::engine::DocumentEngine;
use crate::models::document::Document;
use crate::models::ocr::{Cancellation, ProgressCallback};
use crate::models::task::{
    ConversionError, ConversionResult, ConversionStage, ConversionTask, ErrorCode,
};

/// Hard limits of the MinerU Agent lightweight parse API (no token required,
/// IP rate-limited). Files above these limits must fall back to the local
/// MinerU engine.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
const POLL_INTERVAL: Duration = Duration::from_secs(2);
const POLL_TIMEOUT: Duration = Duration::from_secs(300);

/// Extensions accepted by the MinerU Agent lightweight parse API. Anything
/// else is rejected up front with a friendly error instead of a server `-30002`.
const CLOUD_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "pptx", "xlsx", "png", "jpg", "jpeg", "jp2", "webp", "gif", "bmp",
];

fn cloud_supports_ext(ext: &str) -> bool {
    CLOUD_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

/// Generic envelope of the MinerU Agent API. `code == 0` means success.
#[derive(Debug, Deserialize)]
struct CloudApiEnvelope<T> {
    code: i32,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    data: Option<T>,
}

/// `POST /api/v1/agent/parse/file` response data.
#[derive(Debug, Default, Deserialize)]
struct CloudSubmitData {
    task_id: String,
    /// Signed OSS upload URL; the client PUTs the raw file bytes to it.
    #[serde(default)]
    file_url: Option<String>,
}

/// `GET /api/v1/agent/parse/{task_id}` response data.
#[derive(Debug, Default, Deserialize)]
struct CloudPollData {
    /// One of `waiting-file` / `uploading` / `pending` / `running` / `done` /
    /// `failed`.
    #[serde(default)]
    state: String,
    /// CDN link to the resulting markdown; present when `state == "done"`.
    #[serde(default)]
    markdown_url: Option<String>,
    #[serde(default)]
    err_msg: Option<String>,
    #[serde(default)]
    err_code: Option<i64>,
}

/// Engine backed by the MinerU Agent lightweight parse API (`mineru.net`).
///
/// Temporary fallback used when the local pipeline model is not yet
/// downloaded. It is a pure HTTP client (no local subprocess) and supports
/// only Markdown output, ≤10MB files, ≤20 pages. Requests are IP rate-limited;
/// a 429 surfaces as a retryable error.
pub struct CloudEngine {
    client: reqwest::Client,
    base_url: String,
}

impl CloudEngine {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            // Browser-like UA: the MinerU CDN/WAF can reset connections from
            // clients that send no User-Agent at all.
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default();
        CloudEngine {
            client,
            base_url: "https://mineru.net/api/v1/agent".to_string(),
        }
    }

    /// Submit -> signed upload -> poll -> download the final markdown.
    async fn submit_and_wait(
        &self,
        task: &ConversionTask,
        on_progress: Option<ProgressCallback>,
        cancelled: Option<&Cancellation>,
    ) -> Result<String, ConversionError> {
        let file_path = PathBuf::from(&task.source_path);
        let file_name = file_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "input".to_string());

        if let Some(ext) = file_path.extension().map(|s| s.to_string_lossy().to_string()) {
            if !cloud_supports_ext(&ext) {
                return Err(ConversionError {
                    code: ErrorCode::EngineError,
                    message: format!(
                        "云端解析不支持 .{} 格式，请下载本地模型或使用 PDF/Word/PPT/Excel/图片",
                        ext.to_ascii_lowercase()
                    ),
                    stage: ConversionStage::Queued,
                    retryable: false,
                    page: None,
                });
            }
        }

        let file_size = std::fs::metadata(&file_path)
            .map(|m| m.len())
            .map_err(|e| ConversionError {
                code: ErrorCode::IoError,
                message: format!("读取文件失败: {}", e),
                stage: ConversionStage::Queued,
                retryable: false,
                page: None,
            })?;
        if file_size > MAX_FILE_SIZE {
            return Err(ConversionError {
                code: ErrorCode::EngineError,
                message: format!(
                    "文件大小 {:.1}MB 超过云端解析 10MB 上限，请下载本地模型后本地解析",
                    file_size as f64 / 1024.0 / 1024.0
                ),
                stage: ConversionStage::Queued,
                retryable: false,
                page: None,
            });
        }

        let file_bytes = std::fs::read(&file_path).map_err(|e| ConversionError {
            code: ErrorCode::IoError,
            message: format!("读取文件失败: {}", e),
            stage: ConversionStage::Queued,
            retryable: false,
            page: None,
        })?;

        if let Some(cb) = &on_progress {
            cb(0.05, Some("上传文件".to_string()));
        }

        // 1. Request a signed OSS upload URL for this file.
        let submit_url = format!("{}/parse/file", self.base_url);
        let submit_body = serde_json::json!({
            "file_name": file_name,
            "language": "ch",
            "enable_table": true,
            "enable_formula": true,
            "is_ocr": false,
        });

        let resp = self
            .client
            .post(&submit_url)
            .json(&submit_body)
            .send()
            .await
            .map_err(cloud_send_err)?;

        if resp.status().as_u16() == 429 {
            return Err(cloud_rate_limited());
        }
        if !resp.status().is_success() {
            let status_code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConversionError {
                code: ErrorCode::EngineError,
                message: format!("云端解析提交失败 ({}): {}", status_code, body),
                stage: ConversionStage::ModelLoading,
                retryable: true,
                page: None,
            });
        }

        let envelope: CloudApiEnvelope<CloudSubmitData> =
            resp.json().await.map_err(cloud_json_err)?;
        if envelope.code != 0 {
            return Err(ConversionError {
                code: ErrorCode::EngineError,
                message: format!("云端解析提交失败: {}", envelope.msg),
                stage: ConversionStage::ModelLoading,
                retryable: true,
                page: None,
            });
        }
        let data = envelope.data.ok_or_else(|| ConversionError {
            code: ErrorCode::EngineError,
            message: "云端解析响应缺少数据".to_string(),
            stage: ConversionStage::ModelLoading,
            retryable: true,
            page: None,
        })?;
        let file_url = data.file_url.ok_or_else(|| ConversionError {
            code: ErrorCode::EngineError,
            message: "云端解析未返回上传地址".to_string(),
            stage: ConversionStage::ModelLoading,
            retryable: true,
            page: None,
        })?;

        // 2. PUT raw bytes to the signed OSS URL. Do not set Content-Type —
        // the signed URL carries a signature that must not be altered.
        let put = self.client.put(&file_url).body(file_bytes).send().await;
        match put {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) if resp.status().as_u16() == 429 => return Err(cloud_rate_limited()),
            Ok(resp) => {
                let status_code = resp.status();
                return Err(ConversionError {
                    code: ErrorCode::EngineError,
                    message: format!("上传文件到云端失败 ({})", status_code),
                    stage: ConversionStage::ModelLoading,
                    retryable: true,
                    page: None,
                });
            }
            Err(e) => {
                return Err(ConversionError {
                    code: ErrorCode::EngineError,
                    message: format!("上传文件到云端失败: {}", e),
                    stage: ConversionStage::ModelLoading,
                    retryable: true,
                    page: None,
                });
            }
        }

        if let Some(cb) = &on_progress {
            cb(0.15, Some("等待解析".to_string()));
        }

        // 3. Poll until done/failed or the timeout elapses.
        let poll_url = format!("{}/parse/{}", self.base_url, data.task_id);
        let deadline = Instant::now() + POLL_TIMEOUT;
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
            if Instant::now() >= deadline {
                return Err(ConversionError {
                    code: ErrorCode::EngineError,
                    message: "云端解析超时（300 秒），请稍后重试或下载本地模型".to_string(),
                    stage: ConversionStage::Parsing,
                    retryable: true,
                    page: None,
                });
            }

            let poll = self.client.get(&poll_url).send().await;
            let (state, markdown_url, err_msg, err_code) = match poll {
                Ok(resp) if resp.status().as_u16() == 429 => return Err(cloud_rate_limited()),
                Ok(resp) if resp.status().is_success() => {
                    let envelope: CloudApiEnvelope<CloudPollData> =
                        resp.json().await.map_err(cloud_json_err)?;
                    let data = envelope.data.unwrap_or_default();
                    (data.state, data.markdown_url, data.err_msg, data.err_code)
                }
                Ok(resp) => {
                    let status_code = resp.status();
                    return Err(ConversionError {
                        code: ErrorCode::EngineError,
                        message: format!("查询云端任务状态失败 ({})", status_code),
                        stage: ConversionStage::Parsing,
                        retryable: true,
                        page: None,
                    });
                }
                Err(e) => {
                    return Err(ConversionError {
                        code: ErrorCode::EngineError,
                        message: format!("查询云端任务状态失败: {}", e),
                        stage: ConversionStage::Parsing,
                        retryable: true,
                        page: None,
                    });
                }
            };

            match state.as_str() {
                "done" => {
                    let url = markdown_url.ok_or_else(|| ConversionError {
                        code: ErrorCode::EngineError,
                        message: "云端解析完成但缺少下载地址".to_string(),
                        stage: ConversionStage::Parsing,
                        retryable: false,
                        page: None,
                    })?;
                    return self.download_markdown(&url, on_progress).await;
                }
                "failed" => {
                    let detail = err_msg.unwrap_or_default();
                    let suffix = err_code
                        .map(|c| format!(" (code: {})", c))
                        .unwrap_or_default();
                    let msg = if detail.is_empty() {
                        format!("云端解析失败{}", suffix)
                    } else {
                        format!("{}{}", detail, suffix)
                    };
                    return Err(ConversionError {
                        code: ErrorCode::EngineError,
                        message: msg,
                        stage: ConversionStage::Parsing,
                        retryable: true,
                        page: None,
                    });
                }
                "running" => {
                    if let Some(cb) = &on_progress {
                        cb(0.55, Some("解析中".to_string()));
                    }
                }
                "pending" | "uploading" => {
                    if let Some(cb) = &on_progress {
                        cb(0.3, Some("排队中".to_string()));
                    }
                }
                // `waiting-file` should not normally occur (we upload before
                // polling); treat any remaining state as still busy.
                _ => {}
            }

            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Download the final markdown from the CDN link returned on completion.
    async fn download_markdown(
        &self,
        url: &str,
        on_progress: Option<ProgressCallback>,
    ) -> Result<String, ConversionError> {
        if let Some(cb) = &on_progress {
            cb(0.85, Some("下载结果".to_string()));
        }
        let mut last_err = String::new();
        for attempt in 0..3 {
            let req = self
                .client
                .get(url)
                // Force HTTP/1.1: some CDNs reset HTTP/2 connections from
                // non-browser clients mid-handshake.
                .version(reqwest::Version::HTTP_11)
                // OpenXLab CDN enforces Referer-based hotlink protection.
                .header("Referer", "https://mineru.net/")
                .header("Accept", "text/plain,text/markdown,*/*");
            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    return resp.text().await.map_err(|e| ConversionError {
                        code: ErrorCode::EngineError,
                        message: format!("读取云端结果失败: {}", e),
                        stage: ConversionStage::PostProcessing,
                        retryable: true,
                        page: None,
                    });
                }
                Ok(resp) => {
                    let status_code = resp.status();
                    return Err(ConversionError {
                        code: ErrorCode::EngineError,
                        message: format!("下载云端结果失败 ({})", status_code),
                        stage: ConversionStage::PostProcessing,
                        retryable: true,
                        page: None,
                    });
                }
                Err(e) => {
                    last_err = format!("{} ({})", e, reqwest_chain(&e));
                    tokio::time::sleep(Duration::from_secs(attempt as u64 + 1)).await;
                }
            }
        }
        Err(ConversionError {
            code: ErrorCode::EngineError,
            message: format!("下载云端结果失败: {}", last_err),
            stage: ConversionStage::PostProcessing,
            retryable: true,
            page: None,
        })
    }
}

/// Flatten the reqwest error chain (e.g. `error sending request` ->
/// `client error` -> `connection reset`) so the real cause is visible in the
/// UI instead of only the generic wrapper message.
fn reqwest_chain(e: &reqwest::Error) -> String {
    let mut parts = Vec::new();
    let mut src = e.source();
    while let Some(s) = src {
        parts.push(s.to_string());
        src = s.source();
    }
    if parts.is_empty() {
        e.to_string()
    } else {
        parts.join(" | ")
    }
}

fn cloud_send_err(e: reqwest::Error) -> ConversionError {
    ConversionError {
        code: ErrorCode::EngineError,
        message: format!("连接云端解析服务失败: {}", e),
        stage: ConversionStage::ModelLoading,
        retryable: true,
        page: None,
    }
}

fn cloud_json_err(e: reqwest::Error) -> ConversionError {
    ConversionError {
        code: ErrorCode::EngineError,
        message: format!("解析云端解析响应失败: {}", e),
        stage: ConversionStage::ModelLoading,
        retryable: true,
        page: None,
    }
}

fn cloud_rate_limited() -> ConversionError {
    ConversionError {
        code: ErrorCode::EngineError,
        message: "云端解析请求过于频繁，请稍后重试或下载本地模型".to_string(),
        stage: ConversionStage::ModelLoading,
        retryable: true,
        page: None,
    }
}

#[async_trait::async_trait]
impl DocumentEngine for CloudEngine {
    fn name(&self) -> &str {
        "MinerU 云端"
    }

    fn is_available(&self) -> bool {
        // Presence is assumed; reachability is reported by convert().
        true
    }

    async fn convert(
        &self,
        task: &ConversionTask,
        on_progress: Option<ProgressCallback>,
        cancelled: Option<&Cancellation>,
    ) -> Result<ConversionResult, ConversionError> {
        if let Some(cb) = &on_progress {
            cb(0.02, Some("准备文件".to_string()));
        }

        let markdown = self.submit_and_wait(task, on_progress.clone(), cancelled).await?;

        if let Some(cb) = &on_progress {
            cb(0.9, Some("保存结果".to_string()));
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
        std::fs::write(&out_path, &markdown).map_err(|e| ConversionError {
            code: ErrorCode::IoError,
            message: format!("写入 Markdown 失败: {}", e),
            stage: ConversionStage::Saving,
            retryable: false,
            page: None,
        })?;

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
            output_path: out_path.to_string_lossy().to_string(),
            stats: Some(stats),
        })
    }
}