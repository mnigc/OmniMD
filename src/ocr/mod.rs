pub mod engine;
pub mod pdf_renderer;
pub mod pdf_text_detection;
pub mod postprocess;
pub mod preprocess;
pub mod resources;

pub use engine::PpOcrEngine;
pub use resources::{check_resources, find_det_model, find_rec_dict, find_rec_model, find_ori_model, model_dir};

pub fn get_engine() -> PpOcrEngine {
    PpOcrEngine
}
