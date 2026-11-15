use std::io::Cursor;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::{imageops::FilterType, ImageFormat};

use crate::error::{PasteError, Result};

/// Decodes an image byte slice, returns `(width, height, base64_png_thumbnail)`.
pub fn create_thumbnail_and_dimensions(
    image_bytes: &[u8],
    max_dim: u32,
) -> Result<(u32, u32, String)> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| PasteError::Image(format!("failed to decode image: {e}")))?;
    let width = img.width();
    let height = img.height();

    let thumb = if width > max_dim || height > max_dim {
        img.resize(max_dim, max_dim, FilterType::Triangle)
    } else {
        img
    };

    let mut buf = Cursor::new(Vec::new());
    thumb
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| PasteError::Image(format!("failed to encode thumbnail png: {e}")))?;

    Ok((width, height, BASE64.encode(buf.into_inner())))
}

/// Samples the four corner pixels of an image to estimate a solid background color,
/// then sets matching pixels within `tolerance` (per-channel Euclidean distance) to transparent.
pub fn remove_solid_background(png_bytes: &[u8], tolerance: u8) -> Result<Vec<u8>> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| PasteError::Image(format!("failed to decode image: {e}")))?;
    let mut rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return Err(PasteError::Image("empty image".to_string()));
    }

    let corners = [
        rgba.get_pixel(0, 0).0,
        rgba.get_pixel(w - 1, 0).0,
        rgba.get_pixel(0, h - 1).0,
        rgba.get_pixel(w - 1, h - 1).0,
    ];

    // Average corner RGB as estimated background color
    let bg_r = (corners.iter().map(|c| c[0] as u32).sum::<u32>() / 4) as i32;
    let bg_g = (corners.iter().map(|c| c[1] as u32).sum::<u32>() / 4) as i32;
    let bg_b = (corners.iter().map(|c| c[2] as u32).sum::<u32>() / 4) as i32;
    let tol_sq = (tolerance as i32) * (tolerance as i32) * 3;

    for pixel in rgba.pixels_mut() {
        let dr = pixel[0] as i32 - bg_r;
        let dg = pixel[1] as i32 - bg_g;
        let db = pixel[2] as i32 - bg_b;
        if dr * dr + dg * dg + db * db <= tol_sq {
            pixel[3] = 0;
        }
    }

    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| PasteError::Image(format!("failed to encode png: {e}")))?;
    Ok(out.into_inner())
}
