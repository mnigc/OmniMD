//! Lightweight image preprocessing for OCR (T6).
//!
//! oar-ocr already performs document-orientation classification via its bundled
//! `doc_ori` model, so this stage focuses on pixel-level improvements: a contrast
//! stretch (percentile-based) and a small box-blur denoise. Everything uses the
//! stable `image` 0.25 API so it compiles without extra dependencies.

use image::{DynamicImage, ImageBuffer, Rgb};

use crate::models::task::{ConversionError, ConversionStage, ErrorCode};

/// Preprocess raw image bytes and return PNG bytes ready for OCR.
pub fn preprocess_image(bytes: &[u8]) -> Result<Vec<u8>, ConversionError> {
    let img = image::load_from_memory(bytes).map_err(|e| ConversionError {
        code: ErrorCode::OcrFailed,
        message: format!("Failed to decode image for preprocessing: {}", e),
        stage: ConversionStage::Ocr,
        retryable: true,
        page: None,
    })?;

    let rgb = img.to_rgb8();
    let denoised = reduce_noise(&rgb);
    let enhanced = enhance_contrast(&denoised);

    let out = DynamicImage::ImageRgb8(enhanced);
    let mut png = Vec::new();
    out.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| ConversionError {
            code: ErrorCode::OcrFailed,
            message: format!("Failed to encode preprocessed image: {}", e),
            stage: ConversionStage::Ocr,
            retryable: false,
            page: None,
        })?;
    Ok(png)
}

/// 3x3 box blur (averaging) to suppress salt-and-pepper / scan noise.
fn reduce_noise(img: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let (w, h) = img.dimensions();
    let mut out = ImageBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut r = 0u32;
            let mut g = 0u32;
            let mut b = 0u32;
            let mut n = 0u32;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let p = img.get_pixel(nx as u32, ny as u32);
                    r += p[0] as u32;
                    g += p[1] as u32;
                    b += p[2] as u32;
                    n += 1;
                }
            }
            let p = image::Rgb([
                (r / n) as u8,
                (g / n) as u8,
                (b / n) as u8,
            ]);
            out.put_pixel(x, y, p);
        }
    }
    out
}

/// Percentile-based contrast stretch so faint scans become higher-contrast.
fn enhance_contrast(img: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let (lo_r, hi_r) = channel_percentiles(img, 0);
    let (lo_g, hi_g) = channel_percentiles(img, 1);
    let (lo_b, hi_b) = channel_percentiles(img, 2);

    let stretch = |v: u8, lo: u8, hi: u8| -> u8 {
        if hi <= lo {
            return v;
        }
        let f = (v as f32 - lo as f32) / (hi as f32 - lo as f32);
        (f.clamp(0.0, 1.0) * 255.0).round() as u8
    };

    let mut out = img.clone();
    for p in out.pixels_mut() {
        p[0] = stretch(p[0], lo_r, hi_r);
        p[1] = stretch(p[1], lo_g, hi_g);
        p[2] = stretch(p[2], lo_b, hi_b);
    }
    out
}

/// Compute (low, high) thresholds at the 0.5% / 99.5% percentiles of a channel.
fn channel_percentiles(
    img: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    channel: usize,
) -> (u8, u8) {
    let total = (img.width() * img.height()) as usize;
    if total == 0 {
        return (0, 255);
    }
    let mut hist = [0u32; 256];
    for p in img.pixels() {
        hist[p[channel] as usize] += 1;
    }
    let lo_count = (total as f32 * 0.005) as u32;
    let hi_count = (total as f32 * 0.995) as u32;
    let mut cum = 0u32;
    let mut lo = 0u8;
    let mut hi = 255u8;
    for (v, &c) in hist.iter().enumerate() {
        cum += c;
        if cum >= lo_count && lo == 0 {
            lo = v as u8;
        }
        if cum >= hi_count {
            hi = v as u8;
            break;
        }
    }
    (lo, hi)
}
