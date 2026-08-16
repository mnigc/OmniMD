use std::env;
use std::path::PathBuf;

const OCR_RESOURCES_SUBDIR: &str = "ocr_resources/ppocrv6";

const DET_MODEL: &str = "pp-ocrv6_small_det.onnx";
const REC_MODEL: &str = "pp-ocrv6_small_rec.onnx";
const DICT_FILE: &str = "ppocrv6_dict.txt";
const ORI_MODEL: &str = "pp-lcnet_x1_0_doc_ori.onnx";

/// Returns the base path candidates for bundled resources, ordered by priority:
/// 1. CARGO_MANIFEST_DIR (development)
/// 2. exe parent directory (packaged/installed)
fn bundled_bases() -> Vec<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(OCR_RESOURCES_SUBDIR);
    let mut bases = Vec::new();
    bases.push(manifest);
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            bases.push(parent.join(OCR_RESOURCES_SUBDIR));
        }
    }
    bases
}

/// Find the first base directory that contains all four PP-OCRv6 resource files.
fn find_model_dir() -> Option<PathBuf> {
    for base in bundled_bases() {
        if base.join(DET_MODEL).exists()
            && base.join(REC_MODEL).exists()
            && base.join(DICT_FILE).exists()
            && base.join(ORI_MODEL).exists()
        {
            return Some(base);
        }
    }
    None
}

/// Path to the recognition dictionary file.
pub fn find_rec_dict() -> Option<PathBuf> {
    find_model_dir().map(|d| d.join(DICT_FILE))
}

/// Path to the detection model file.
pub fn find_det_model() -> Option<PathBuf> {
    find_model_dir().map(|d| d.join(DET_MODEL))
}

/// Path to the recognition model file.
pub fn find_rec_model() -> Option<PathBuf> {
    find_model_dir().map(|d| d.join(REC_MODEL))
}

/// Path to the document orientation classification model file.
pub fn find_ori_model() -> Option<PathBuf> {
    find_model_dir().map(|d| d.join(ORI_MODEL))
}

/// Check whether all four PP-OCRv6 resource files exist.
pub fn check_resources() -> bool {
    find_model_dir().is_some()
}

/// Return the model directory path if available.
pub fn model_dir() -> Option<PathBuf> {
    find_model_dir()
}