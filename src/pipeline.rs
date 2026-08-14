use std::fs;
use std::path::Path;

use crate::converters::get_converter;
use crate::file_utils;
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

            let resolved_path = file_utils::resolve_unique_path(asset_path);

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

pub async fn convert_batch<F>(
    tasks: Vec<ConversionTask>,
    concurrency: usize,
    mut on_task_completed: F,
) -> Vec<ConversionResult>
where
    F: FnMut(&ConversionResult),
{
    use futures::stream::{FuturesUnordered, StreamExt};
    use std::collections::HashMap;

    let concurrency = concurrency.max(1);
    let mut task_iter = tasks.into_iter().enumerate();
    let mut futures: FuturesUnordered<
        tokio::task::JoinHandle<(usize, String, Result<ConversionResult, ConversionError>)>,
    > = FuturesUnordered::new();

    let spawn_task = |index: usize, task: ConversionTask| {
        let task_id = task.id.clone();
        tokio::spawn(async move {
            let converted = convert_file(&task).await;
            (index, task_id, converted)
        })
    };

    for _ in 0..concurrency {
        if let Some((index, task)) = task_iter.next() {
            futures.push(spawn_task(index, task));
        }
    }

    let mut by_index: HashMap<usize, ConversionResult> = HashMap::new();
    let mut unresolvable: Vec<ConversionResult> = Vec::new();

    while let Some(handle) = futures.next().await {
        match handle {
            Ok((index, _, Ok(result))) => {
                on_task_completed(&result);
                by_index.insert(index, result);
            }
            Ok((index, task_id, Err(err))) => {
                let result = ConversionResult {
                    task_id,
                    markdown: String::new(),
                    document: Document::new("unknown", "unknown", 0),
                    assets: Vec::new(),
                    errors: vec![err],
                };
                on_task_completed(&result);
                by_index.insert(index, result);
            }
            Err(e) => {
                let result = ConversionResult {
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
                };
                on_task_completed(&result);
                unresolvable.push(result);
            }
        }

        if let Some((index, task)) = task_iter.next() {
            futures.push(spawn_task(index, task));
        }
    }

    let mut ordered: Vec<(usize, ConversionResult)> = by_index.into_iter().collect();
    ordered.sort_by_key(|(index, _)| *index);
    let mut results: Vec<ConversionResult> = ordered.into_iter().map(|(_, r)| r).collect();
    results.extend(unresolvable);
    results
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_punctuation() && c != '.' && c != '-' && c != '_' { '_' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn convert_batch_processes_all_tasks_within_concurrency() {
        let dir = std::env::temp_dir().join(format!(
            "omnid_batch_src_{}",
            std::process::id()
        ));
        let out = std::env::temp_dir().join(format!(
            "omnid_batch_out_{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();

        let count = 8;
        let mut tasks = Vec::new();
        for i in 0..count {
            let src = dir.join(format!("file{}.txt", i));
            fs::write(&src, format!("content {}", i)).unwrap();
            let output = out.join(format!("file{}.md", i));
            tasks.push(ConversionTask::new(
                src.to_str().unwrap(),
                output.to_str().unwrap(),
            ));
        }

        let results = convert_batch(tasks, 2, |_| {}).await;

        fs::remove_dir_all(&dir).ok();
        fs::remove_dir_all(&out).ok();

        assert_eq!(results.len(), count, "all tasks must be consumed");
        for (i, result) in results.iter().enumerate() {
            assert!(result.errors.is_empty(), "task {} should succeed", i);
            assert!(
                result.markdown.contains(&format!("content {}", i)),
                "task {} result should match input order",
                i
            );
        }
    }
}
