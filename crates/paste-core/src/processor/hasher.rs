use sha2::{Digest, Sha256};

use crate::models::NewClipboardPayload;

/// Computes a deterministic SHA-256 checksum across clipboard representations
/// so identical copies deduplicate cleanly regardless of source app or timestamp.
pub fn compute_checksum(payload: &NewClipboardPayload) -> String {
    let mut hasher = Sha256::new();

    if let Some(ref text) = payload.plain_text {
        hasher.update(b"text:");
        hasher.update((text.len() as u64).to_le_bytes());
        hasher.update(text.as_bytes());
    }
    if let Some(ref png) = payload.image_png {
        hasher.update(b"image:");
        hasher.update((png.len() as u64).to_le_bytes());
        hasher.update(png);
    }
    if !payload.file_paths.is_empty() {
        hasher.update(b"files:");
        for p in &payload.file_paths {
            hasher.update((p.len() as u64).to_le_bytes());
            hasher.update(p.as_bytes());
        }
    }
    if payload.plain_text.is_none()
        && payload.image_png.is_none()
        && payload.file_paths.is_empty()
    {
        if let Some(ref html) = payload.html_data {
            hasher.update(b"html:");
            hasher.update(html.as_bytes());
        }
        if let Some(ref rtf) = payload.rtf_data {
            hasher.update(b"rtf:");
            hasher.update(rtf);
        }
    }

    hex::encode(hasher.finalize())
}
