use std::fs;
use std::path::{Path, PathBuf};

use crate::converters::get_converter;
use crate::models::converter::Converter;
use crate::models::{
    document::Document,
    task::{
        ConversionError, ConversionResult, ConversionStage, ConversionTask, ErrorCode,
    },
};

const BUNDLE_ASSET_DIR: &str = "assets";

pub async fn convert_file(task: &ConversionTask) -> Result<ConversionResult, ConversionError> {
    let converter = get_converter();
    let bytes = fs::read(&task.source_path).map_err(|e| ConversionError {
        code: ErrorCode::IoError,
        message: format!("Failed to read file: {}", e),
        stage: ConversionStage::DetectingFormat,
        retryable: true,
    })?;

    let _detected_format = converter
        .detect_format(&bytes)
        .unwrap_or_else(|| "unknown".to_string());

    let mut result = converter.convert(&bytes)?;
    result.task_id = task.id.clone();

    let output_path = Path::new(&task.output_path);
    let output_dir = output_path.parent().unwrap_or_else(|| Path::new("."));

    if !output_dir.exists() {
        fs::create_dir_all(output_dir).map_err(|e| ConversionError {
            code: ErrorCode::IoError,
            message: format!("Failed to create output directory: {}", e),
            stage: ConversionStage::Saving,
            retryable: false,
        })?;
    }

    fs::write(output_path, &result.markdown).map_err(|e| ConversionError {
        code: ErrorCode::IoError,
        message: format!("Failed to write markdown: {}", e),
        stage: ConversionStage::Saving,
        retryable: true,
    })?;

    let asset_dir = output_dir.join(BUNDLE_ASSET_DIR);
    if !result.assets.is_empty() {
        fs::create_dir_all(&asset_dir).map_err(|e| ConversionError {
            code: ErrorCode::IoError,
            message: format!("Failed to create assets directory: {}", e),
            stage: ConversionStage::Saving,
            retryable: false,
        })?;

        for (i, asset) in result.assets.iter().enumerate() {
            let safe_name = sanitize_filename(&asset.name);
            let asset_path = if safe_name.is_empty() {
                asset_dir.join(format!("asset-{:03}", i))
            } else {
                asset_dir.join(safe_name)
            };

            let resolved_path = resolve_unique_file_path(asset_path);

            fs::write(&resolved_path, &asset.bytes).map_err(|e| ConversionError {
                code: ErrorCode::IoError,
                message: format!("Failed to write asset {}: {}", asset.name, e),
                stage: ConversionStage::Saving,
                retryable: true,
            })?;
        }
    }

    Ok(result)
}

pub async fn convert_batch(
    tasks: Vec<ConversionTask>,
    concurrency: usize,
) -> Vec<ConversionResult> {
    use futures::stream::{FuturesUnordered, StreamExt};

    let mut futures: FuturesUnordered<tokio::task::JoinHandle<(String, ConversionResult)>> =
        FuturesUnordered::new();

    let concurrency = concurrency.max(1);

    for task in tasks {
        let handle = tokio::spawn(async move {
            let result = convert_file(&task).await;
            match result {
                Ok(r) => (task.id.clone(), r),
                Err(err) => (
                    task.id.clone(),
                    ConversionResult {
                        task_id: task.id,
                        markdown: String::new(),
                        document: Document::new("unknown", "unknown", 0),
                        assets: Vec::new(),
                        errors: vec![err],
                    },
                ),
            }
        });
        futures.push(handle);
    }

    let mut active = 0usize;
    let mut results: Vec<ConversionResult> = Vec::with_capacity(concurrency);

    while active < concurrency {
        active += 1;
        if let Some(handle) = futures.next().await {
            match handle {
                Ok((_task_id, result)) => results.push(result),
                Err(e) => {
                    results.push(ConversionResult {
                        task_id: String::new(),
                        markdown: String::new(),
                        document: Document::new("unknown", "unknown", 0),
                        assets: Vec::new(),
                        errors: vec![ConversionError {
                            code: ErrorCode::IoError,
                            message: format!("Task panicked: {}", e),
                            stage: ConversionStage::Extracting,
                            retryable: true,
                        }],
                    });
                }
            }
        }
        active -= 1;
    }

    results
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_punctuation() && c != '.' && c != '-' && c != '_' { '_' } else { c })
        .collect()
}

fn resolve_unique_file_path(initial: PathBuf) -> PathBuf {
    if !initial.exists() {
        return initial;
    }

    let parent = initial
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let stem = initial
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "asset".to_string());
    let extension = initial
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();

    let mut counter = 1;
    loop {
        let path = parent.join(format!("{}-{}{}", stem, counter, extension));
        if !path.exists() {
            return path;
        }
        counter += 1;
    }
}
