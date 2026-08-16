use std::sync::Mutex;

use oar_ocr::oarocr::{OAROCR, OAROCRBuilder};
use oar_ocr::utils::image::load_image_from_memory;

use crate::models::ocr::{Cancellation, OcrConfig, OcrResult, OcrBlock, ProgressCallback};
use crate::models::task::{ConversionError, ConversionStage, ErrorCode};
use crate::models::OcrEngine;

use super::resources;

/// Lazily-initialized singleton PP-OCRv6 engine.
struct PpOcrEngineInner {
    ocr: OAROCR,
}

static ENGINE: once_cell::sync::Lazy<Mutex<Option<PpOcrEngineInner>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

pub struct PpOcrEngine;

impl OcrEngine for PpOcrEngine {
    fn name(&self) -> &str {
        "ppocrv6"
    }

    fn is_available(&self) -> bool {
        resources::check_resources()
    }

    fn recognize_image(
        &self,
        image: &[u8],
        _config: &OcrConfig,
        on_progress: Option<ProgressCallback>,
        cancelled: Option<&Cancellation>,
    ) -> Result<OcrResult, ConversionError> {
        if !self.is_available() {
            return Err(ConversionError {
                code: ErrorCode::OcrFailed,
                message: "OCR engine not available. PP-OCRv6 resources are missing.".to_string(),
                stage: ConversionStage::Ocr,
                retryable: false,
                page: None,
            });
        }

        let mut engine_guard = ENGINE.lock().map_err(|e| ConversionError {
            code: ErrorCode::OcrFailed,
            message: format!("Engine lock poisoned: {}", e),
            stage: ConversionStage::Ocr,
            retryable: false,
            page: None,
        })?;

        if engine_guard.is_none() {
            let inner = build_engine()?;
            *engine_guard = Some(inner);
        }

        let engine = engine_guard.as_ref().unwrap();

        if let Some(c) = cancelled {
            if c.cancelled() {
                return Err(cancelled_error());
            }
        }

        if let Some(ref cb) = on_progress {
            cb(0.1, None);
        }

        let rgb_image = load_image_from_memory(image).map_err(|e| ConversionError {
            code: ErrorCode::OcrFailed,
            message: format!("Failed to decode image for OCR: {}", e),
            stage: ConversionStage::Ocr,
            retryable: true,
            page: None,
        })?;
        let img_w = rgb_image.width();
        let img_h = rgb_image.height();

        if let Some(ref cb) = on_progress {
            cb(0.3, None);
        }

        if let Some(c) = cancelled {
            if c.cancelled() {
                return Err(cancelled_error());
            }
        }

        let results = engine.ocr.predict(vec![rgb_image]).map_err(|e| ConversionError {
            code: ErrorCode::OcrFailed,
            message: format!("PP-OCRv6 inference failed: {}", e),
            stage: ConversionStage::Ocr,
            retryable: true,
            page: None,
        })?;

        if let Some(ref cb) = on_progress {
            cb(0.9, None);
        }

        let page_result = match results.into_iter().next() {
            Some(r) => r,
            None => {
                tracing::warn!(
                    "PP-OCRv6 returned no results for image ({} bytes, {}x{})",
                    image.len(),
                    img_w,
                    img_h
                );
                return Ok(OcrResult {
                    page: 1,
                    width: 0,
                    height: 0,
                    blocks: Vec::new(),
                });
            }
        };

        let blocks: Vec<OcrBlock> = page_result
            .text_regions
            .iter()
            .enumerate()
            .map(|(i, region)| {
                let (text, confidence) = region
                    .text_with_confidence()
                    .unwrap_or(("", 0.0));

                let (x_min, y_min, x_max, y_max) = region.bounding_box.aabb();

                OcrBlock {
                    block_type: "text".to_string(),
                    text: text.to_string(),
                    confidence,
                    bbox: [x_min, y_min, x_max, y_max],
                    page: 1,
                    order: i as u32 + 1,
                }
            })
            .collect();

        if let Some(ref cb) = on_progress {
            cb(1.0, None);
        }

        Ok(OcrResult {
            page: 1,
            width: 0,
            height: 0,
            blocks,
        })
    }
}

fn build_engine() -> Result<PpOcrEngineInner, ConversionError> {
    let det_model = resources::find_det_model().ok_or_else(|| ConversionError {
        code: ErrorCode::OcrFailed,
        message: "PP-OCRv6 detection model not found.".to_string(),
        stage: ConversionStage::Ocr,
        retryable: false,
        page: None,
    })?;

    let rec_model = resources::find_rec_model().ok_or_else(|| ConversionError {
        code: ErrorCode::OcrFailed,
        message: "PP-OCRv6 recognition model not found.".to_string(),
        stage: ConversionStage::Ocr,
        retryable: false,
        page: None,
    })?;

    let dict_path = resources::find_rec_dict().ok_or_else(|| ConversionError {
        code: ErrorCode::OcrFailed,
        message: "PP-OCRv6 dictionary not found.".to_string(),
        stage: ConversionStage::Ocr,
        retryable: false,
        page: None,
    })?;

    let det_path = det_model.to_string_lossy().to_string();
    let rec_path = rec_model.to_string_lossy().to_string();
    let dict_str = dict_path.to_string_lossy().to_string();

    let mut builder = OAROCRBuilder::new(det_path.as_str(), rec_path.as_str(), dict_str.as_str());

    if let Some(ori_model) = resources::find_ori_model() {
        let ori_path = ori_model.to_string_lossy().to_string();
        builder = builder.with_document_image_orientation_classification(ori_path.as_str());
    }

    let ocr = builder
        .image_batch_size(1)
        .region_batch_size(32)
        .build()
        .map_err(|e| ConversionError {
            code: ErrorCode::OcrFailed,
            message: format!("Failed to build PP-OCRv6 engine: {}", e),
            stage: ConversionStage::Ocr,
            retryable: false,
            page: None,
        })?;

    Ok(PpOcrEngineInner { ocr })
}

fn cancelled_error() -> ConversionError {
    ConversionError {
        code: ErrorCode::Cancelled,
        message: "?????".to_string(),
        stage: ConversionStage::Ocr,
        retryable: false,
        page: None,
    }
}
