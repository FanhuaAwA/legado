use serde::Serialize;
use thiserror::Error;

pub mod error;

pub use error::AppError;

#[derive(Debug, Error)]
pub enum ReaderCoreError {
    #[error(transparent)]
    App(#[from] AppError),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Message(String),
}

impl ReaderCoreError {
    pub fn code(&self) -> &'static str {
        match self {
            ReaderCoreError::App(AppError::NotFound(_)) => "SOURCE_NOT_FOUND",
            ReaderCoreError::App(AppError::BadRequest(_)) => "SOURCE_PARSE_FAILED",
            ReaderCoreError::App(AppError::Db(_)) | ReaderCoreError::Sqlx(_) => "DB_ERROR",
            ReaderCoreError::App(AppError::Http(_)) | ReaderCoreError::Http(_) => "NETWORK_FAILED",
            ReaderCoreError::App(AppError::Internal(_))
            | ReaderCoreError::Anyhow(_)
            | ReaderCoreError::Io(_)
            | ReaderCoreError::Json(_)
            | ReaderCoreError::Message(_) => "IO_ERROR",
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            ReaderCoreError::App(AppError::Http(_)) | ReaderCoreError::Http(_)
        )
    }

    pub fn into_command_error(self) -> CommandError {
        CommandError {
            code: self.code().to_string(),
            message: self.to_string(),
            detail: Some(format!("{self:?}")),
            retryable: self.retryable(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
    pub retryable: bool,
}
