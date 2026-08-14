use super::task::ConversionError;

pub trait OcrEngine {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn detect_language(&self, image: &[u8]) -> Result<String, ConversionError>;
    fn ocr(&self, image: &[u8], lang: &str) -> Result<String, ConversionError>;
}
