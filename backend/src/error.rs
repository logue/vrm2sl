use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Processing failed: {0}")]
    Process(String),

    #[error("Filesystem error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("Path conversion error")]
    PathConversion,

    #[error("General error: {0}")]
    General(String),
}

/// Tauri commands return `String` errors, so convert `AppError` accordingly.
impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.to_string()
    }
}
