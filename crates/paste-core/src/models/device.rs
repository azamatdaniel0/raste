use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i64)]
pub enum DeviceType {
    Mac = 0,
    IPhone = 1,
    IPad = 2,
    Windows = 3,
    Linux = 4,
}

impl DeviceType {
    pub fn from_raw(value: i64) -> Self {
        match value {
            0 => Self::Mac,
            1 => Self::IPhone,
            2 => Self::IPad,
            3 => Self::Windows,
            4 => Self::Linux,
            _ => Self::current_os(),
        }
    }

    pub fn current_os() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// Device metadata (`PasteCore.DeviceEntity`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEntity {
    pub identifier: String,
    pub name: String,
    pub app_version: String,
    pub device_type: DeviceType,
}
