use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("prompt not found: {0}")]
    NotFound(String),

    #[error("invalid prompt: {0}")]
    Invalid(String),

    #[error("invalid tab: {0}")]
    InvalidTab(String),

    #[error("tab not found: {0}")]
    TabNotFound(String),

    #[error("multiple tabs exist; specify --tab <name> (available: {0})")]
    TabRequired(String),

    #[error("unsupported storage schema: {0}")]
    UnsupportedSchema(u32),

    #[error("storage error: {0}")]
    Storage(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
