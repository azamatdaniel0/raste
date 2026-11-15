use serde::{Deserialize, Serialize};

/// Combined representation of `ListEntity` and `ListMetadataEntity`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pinboard {
    pub identifier: String,
    pub name: String,
    pub raw_type: i64,
    pub color_hex: String,
    pub icon_name: String,
    pub tab_index: i64,
    pub item_count: usize,
    pub created_at: i64,
    pub updated_at: i64,
}
