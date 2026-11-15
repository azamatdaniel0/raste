use serde::{Deserialize, Serialize};

/// Cached originating application metadata (`PasteCore.ApplicationEntity`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationEntity {
    pub bundle_identifier: String,
    pub name: String,
    pub raw_color: String,
    pub raw_icon: Option<Vec<u8>>,
}
