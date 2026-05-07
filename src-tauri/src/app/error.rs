use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("io error while {context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("json error while {context}: {source}")]
    Json {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid settings: {message}")]
    InvalidSettings { message: String },
    #[error("audio backend error: {message}")]
    Audio { message: String },
    #[error("realtime session error: {message}")]
    RealtimeSession { message: String },
    #[error("offline job error: {message}")]
    OfflineJob { message: String },
    #[error("unsupported feature: {message}")]
    Unsupported { message: String },
}

impl AppError {
    pub fn io(context: &'static str, source: std::io::Error) -> Self {
        Self::Io { context, source }
    }

    pub fn json(context: &'static str, source: serde_json::Error) -> Self {
        Self::Json { context, source }
    }

    pub fn invalid_settings(message: impl Into<String>) -> Self {
        Self::InvalidSettings {
            message: message.into(),
        }
    }

    pub fn audio(message: impl Into<String>) -> Self {
        Self::Audio {
            message: message.into(),
        }
    }

    pub fn realtime_session(message: impl Into<String>) -> Self {
        Self::RealtimeSession {
            message: message.into(),
        }
    }

    pub fn offline_job(message: impl Into<String>) -> Self {
        Self::OfflineJob {
            message: message.into(),
        }
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported {
            message: message.into(),
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "io_error",
            Self::Json { .. } => "json_error",
            Self::InvalidSettings { .. } => "invalid_settings",
            Self::Audio { .. } => "audio_error",
            Self::RealtimeSession { .. } => "realtime_session_error",
            Self::OfflineJob { .. } => "offline_job_error",
            Self::Unsupported { .. } => "unsupported_feature",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
pub type ApiResult<T> = Result<T, ApiError>;
