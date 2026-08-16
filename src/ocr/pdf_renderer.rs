//! PDF rendering and embedded-image utilities (T7, T11).
//!
//! Holds PDFium-based page rendering (used by the hybrid PDF path) and the
//! legacy PDF embedded-image / BMP helpers that previously lived in
//! `engine.rs`. The image-format detector also lives here so the engine module
//! stays focused on OCR inference.

use lopdf::Document;
use pdfium_bundled::pdfium_render::prelude::*;

use crate::models::task::{ConversionError, ConversionStage, ErrorCode};

/// Bind to the PDFium library, handling the case where it is already bound.
///
/// `pdfium_bundled::bind_pdfium_silent()` calls `Pdfium::bind_to_library()`
/// under the hood, which initializes a global `OnceCell` on the first call.
/// A second call returns `PdfiumLibraryBindingsAlreadyInitialized`. We catch
/// that and reuse the existing bindings via `Pdfium::default()`, which is safe
/// because `Pdfium::default()` checks the same global cell and short-circuits
/// to a fresh `Pdfium` that shares the already-bound library.
pub fn get_pdfium() -> Result<Pdfium, ConversionError> {
    match pdfium_bundled::bind_pdfium_silent() {
        Ok(pdfium) => Ok(pdfium),
        Err(e) => {
            let reason = e.to_string();
            if reason.contains("AlreadyInitialized") {
                tracing::debug!("PDFium already initialized, reusing existing bindings");
                Ok(Pdfium::default())
            } else {
                Err(ConversionError {
                    code: ErrorCode::OcrFailed,
                    message: format!("Failed to initialize PDF renderer: {}", e),
                    stage: ConversionStage::Ocr,
                    retryable: false,
                    page: None,
                })
            }
        }
    }
}

/// Render a single PDF page to PNG bytes at the given DPI. The caller is
/// responsible for freeing the returned buffer; PDFium page/bitmap handles are
/// released inside this function to bound memory on multi-page runs.
pub fn render_pdf_page_to_png(
    bytes: &[u8],
    page_index: usize,
    dpi: u32,
) -> Result<Vec<u8>, ConversionError> {
    let pdfium = get_pdfium()?;

    let document = pdfium.load_pdf_from_byte_slice(bytes, None).map_err(|e| ConversionError {
        code: ErrorCode::OcrFailed,
        message: format!("Failed to load PDF: {}", e),
        stage: ConversionStage::Ocr,
        retryable: false,
        page: Some((page_index + 1) as u32),
    })?;

    let page = document
        .pages()
        .get(page_index as i32)
        .map_err(|e| ConversionError {
        code: ErrorCode::OcrFailed,
        message: format!("Failed to get PDF page {}: {}", page_index + 1, e),
        stage: ConversionStage::Ocr,
        retryable: false,
        page: Some((page_index + 1) as u32),
    })?;

    let width_pts = page.width();
    let height_pts = page.height();
    let dpi_scale = dpi as f32 / 72.0;
    let target_width = (width_pts.value * dpi_scale) as i32;
    let target_height = (height_pts.value * dpi_scale) as i32;

    let render_config = PdfRenderConfig::new()
        .set_target_width(target_width)
        .set_target_height(target_height);

    let bitmap = page.render_with_config(&render_config).map_err(|e| ConversionError {
        code: ErrorCode::OcrFailed,
        message: format!("Failed to render PDF page {}: {}", page_index + 1, e),
        stage: ConversionStage::Ocr,
        retryable: false,
        page: Some((page_index + 1) as u32),
    })?;

    let image = bitmap.as_image().map_err(|_| ConversionError {
        code: ErrorCode::OcrFailed,
        message: "Failed to convert PDF page render to image.".to_string(),
        stage: ConversionStage::Ocr,
        retryable: false,
        page: Some((page_index + 1) as u32),
    })?;

    let mut png_bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .map_err(|_| ConversionError {
            code: ErrorCode::OcrFailed,
            message: "Failed to encode PDF page as PNG.".to_string(),
            stage: ConversionStage::Ocr,
            retryable: false,
            page: Some((page_index + 1) as u32),
        })?;

    drop(bitmap);
    drop(page);

    Ok(png_bytes)
}

/// True if the byte slice starts with a PDF header.
pub fn is_pdf(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == 0x25 && bytes[1] == 0x50 && bytes[2] == 0x44 && bytes[3] == 0x46
}

/// Hold image info extracted from a PDF page, with decompressed pixel data
/// ready to be written as a standalone image file.
pub struct PdfPageImage {
    /// 1-based page number this image belongs to (0 when unknown).
    pub page: u32,
    /// Width in pixels
    pub width: i64,
    /// Height in pixels
    pub height: i64,
    /// Decompressed pixel data (raw BGR/BGRA/Gray)
    pub data: Vec<u8>,
    /// Number of bytes per pixel (1=gray, 3=RGB, 4=CMYK/RGBA)
    pub bytes_per_pixel: u8,
    /// Suggested file extension
    pub ext: &'static str,
}

/// Extract embedded images from a PDF, ordered by page number (1-based).
pub fn extract_embedded_images(doc: &Document) -> Vec<PdfPageImage> {
    use std::collections::HashSet;

    let mut images: Vec<PdfPageImage> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();

    for (page_number, page_id) in doc.get_pages() {
        let pdf_images = match doc.get_page_images(page_id) {
            Ok(imgs) => imgs,
            Err(_) => continue,
        };

        for pdf_img in pdf_images {
            if !seen.insert(pdf_img.id.0) {
                continue;
            }

            let stream = match doc.get_object(pdf_img.id) {
                Ok(obj) => match obj.as_stream() {
                    Ok(s) => s,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            let is_jpeg = pdf_img
                .filters
                .as_ref()
                .map(|fs| fs.iter().any(|f| f == "DCTDecode"))
                .unwrap_or(false);

            let data = match stream.decompressed_content() {
                Ok(d) => d,
                Err(_) => {
                    if is_jpeg {
                        stream.content.clone()
                    } else {
                        continue;
                    }
                }
            };

            if data.is_empty() || pdf_img.width <= 0 || pdf_img.height <= 0 {
                continue;
            }

            let bytes_per_pixel = match pdf_img.color_space.as_deref() {
                Some("DeviceGray") => 1,
                Some("DeviceRGB") => 3,
                Some("DeviceCMYK") => 4,
                _ => 3,
            };

            let ext = if is_jpeg { "jpg" } else { "bmp" };

            images.push(PdfPageImage {
                page: page_number,
                width: pdf_img.width,
                height: pdf_img.height,
                data,
                bytes_per_pixel,
                ext,
            });
        }
    }

    images
}

/// Encode raw pixel data as a BMP file (24-bit or 32-bit).
pub fn encode_as_bmp(img: &PdfPageImage) -> Vec<u8> {
    let w = img.width as u32;
    let h = img.height as u32;
    let bpp = img.bytes_per_pixel;

    let row_size = ((w * bpp as u32) + 3) & !3;
    let pixel_data_size = row_size * h;
    let header_size: u32 = 14 + 40;
    let file_size = header_size + pixel_data_size;

    let mut bmp = Vec::with_capacity(file_size as usize);

    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&[0u8; 4]);
    bmp.extend_from_slice(&header_size.to_le_bytes());

    bmp.extend_from_slice(&(40u32).to_le_bytes());
    bmp.extend_from_slice(&w.to_le_bytes());
    bmp.extend_from_slice(&h.to_le_bytes());
    bmp.extend_from_slice(&(1u16).to_le_bytes());
    let bit_count: u16 = (bpp as u16) * 8;
    bmp.extend_from_slice(&bit_count.to_le_bytes());
    bmp.extend_from_slice(&[0u8; 4]);
    bmp.extend_from_slice(&pixel_data_size.to_le_bytes());
    bmp.extend_from_slice(&[0u8; 8]);
    bmp.extend_from_slice(&[0u8; 4]);
    bmp.extend_from_slice(&[0u8; 4]);

    let src = &img.data;
    let src_row_size = (w as usize) * (bpp as usize);

    for y in (0..h).rev() {
        let src_start = (y as usize) * src_row_size;
        let src_end = std::cmp::min(src_start + src_row_size, src.len());
        let row_data = &src[src_start..src_end];

        let mut padded_row = vec![0u8; row_size as usize];
        let row_pixels = std::cmp::min(row_data.len(), (w as usize) * (bpp as usize));

        if bpp == 3 {
            for i in (0..row_pixels).step_by(3) {
                if i + 2 < row_pixels {
                    padded_row[i] = row_data[i + 2];
                    padded_row[i + 1] = row_data[i + 1];
                    padded_row[i + 2] = row_data[i];
                }
            }
        } else if bpp == 1 {
            for i in 0..std::cmp::min(row_pixels, w as usize) {
                let gray = row_data[i];
                let dst = i * 3;
                padded_row[dst] = gray;
                padded_row[dst + 1] = gray;
                padded_row[dst + 2] = gray;
            }
        } else {
            let copy_len = std::cmp::min(row_pixels, padded_row.len());
            padded_row[..copy_len].copy_from_slice(&row_data[..copy_len]);
        }

        bmp.extend_from_slice(&padded_row);
    }

    bmp
}

/// Detect the image format from magic bytes and return the correct file extension.
pub fn detect_image_format(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 8 && bytes[0] == 0x89 && bytes[1] == 0x50 && bytes[2] == 0x4E && bytes[3] == 0x47
    {
        return "png";
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        return "jpg";
    }
    if bytes.len() >= 2 && bytes[0] == 0x42 && bytes[1] == 0x4D {
        return "bmp";
    }
    if bytes.len() >= 4
        && ((bytes[0] == 0x49 && bytes[1] == 0x49 && bytes[2] == 0x2A && bytes[3] == 0x00)
            || (bytes[0] == 0x4D && bytes[1] == 0x4D && bytes[2] == 0x00 && bytes[3] == 0x2A))
    {
        return "tiff";
    }
    "png"
}
