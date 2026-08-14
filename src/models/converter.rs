use super::task::{ConversionError, ConversionResult};

pub trait Converter {
    fn name(&self) -> &str;
    fn supports(&self, format: &str) -> bool;
    fn convert(&self, bytes: &[u8]) -> Result<ConversionResult, ConversionError>;
    fn detect_format(&self, bytes: &[u8]) -> Option<String>;
}
