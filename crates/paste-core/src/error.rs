use thiserror::Error;

/// Typed errors returned by `paste-core` operations.
#[derive(Debug, Error)]
pub enum PasteError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("image processing error: {0}")]
    Image(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("confirmation required: {0}")]
    ConfirmationRequired(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, PasteError>;
