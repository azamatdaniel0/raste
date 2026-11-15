use serde::{Deserialize, Serialize};

use super::preview::ItemPreview;

/// Content classification matching `PasteCore.ItemEntity.rawType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i64)]
pub enum ItemType {
    Text = 0,
    Link = 1,
    Image = 2,
    Color = 3,
    Files = 4,
    Unknown = 5,
}

impl ItemType {
    pub fn from_raw(value: i64) -> Self {
        match value {
            0 => Self::Text,
            1 => Self::Link,
            2 => Self::Image,
            3 => Self::Color,
            4 => Self::Files,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Link => "link",
            Self::Image => "image",
            Self::Color => "color",
            Self::Files => "file",
            Self::Unknown => "unknown",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Link => "Link",
            Self::Image => "Image",
            Self::Color => "Color",
            Self::Files => "Files",
            Self::Unknown => "Other",
        }
    }

    pub fn from_str_filter(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" => Some(Self::Text),
            "link" | "url" => Some(Self::Link),
            "image" | "img" => Some(Self::Image),
            "color" | "colour" => Some(Self::Color),
            "file" | "files" => Some(Self::Files),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// Lightweight clipboard card metadata (`PasteCore.ItemEntity`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemEntity {
    pub identifier: String,
    pub title: String,
    pub checksum: String,
    pub item_type: ItemType,
    pub preview: ItemPreview,
    pub timestamp: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub display_order_in_pinboard: i64,
    pub source_app_id: Option<String>,
    pub source_app_name: Option<String>,
    pub source_app_color: Option<String>,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub pinboard_id: Option<String>,
    pub is_confidential: bool,
}

/// Heavy clipboard payload separated from metadata (`PasteCore.ItemDataEntity`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDataEntity {
    pub item_id: String,
    pub plain_text: Option<String>,
    pub rtf_data: Option<Vec<u8>>,
    pub html_data: Option<String>,
    pub image_png: Option<Vec<u8>>,
    pub file_paths: Vec<String>,
    pub raw_representations_json: Option<String>,
}

/// Input payload captured from OS clipboard or created manually / via MCP.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewClipboardPayload {
    pub plain_text: Option<String>,
    pub rtf_data: Option<Vec<u8>>,
    pub html_data: Option<String>,
    pub image_png: Option<Vec<u8>>,
    pub file_paths: Vec<String>,
    pub source_app_id: Option<String>,
    pub source_app_name: Option<String>,
    pub source_app_color: Option<String>,
    pub source_app_icon: Option<Vec<u8>>,
    pub device_id: Option<String>,
    pub pinboard_id: Option<String>,
    pub custom_title: Option<String>,
}
