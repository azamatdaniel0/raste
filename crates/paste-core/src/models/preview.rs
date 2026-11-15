use serde::{Deserialize, Serialize};

/// Pre-computed card preview metadata stored inside `ItemEntity.raw_preview`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemPreview {
    pub snippet: String,
    pub char_count: usize,
    pub word_count: usize,
    pub line_count: usize,
    pub language_hint: Option<String>,
    pub link_url: Option<String>,
    pub link_domain: Option<String>,
    pub color_hex: Option<String>,
    pub color_rgba: Option<[u8; 4]>,
    pub color_hsl: Option<(f32, f32, f32)>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub image_format: Option<String>,
    pub image_size_bytes: Option<usize>,
    pub thumbnail_png_b64: Option<String>,
    pub file_names: Vec<String>,
    pub file_count: usize,
}
