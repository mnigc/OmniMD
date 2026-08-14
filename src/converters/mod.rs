pub mod anydoc_converter;

pub use anydoc_converter::AnyDocConverter;

pub fn get_converter() -> AnyDocConverter {
    AnyDocConverter::new()
}
