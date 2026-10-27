use serde::ser::{Serialize, SerializeStruct, Serializer};

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    BadRequest { code: String, message: String },
    #[error("{message}")]
    Unauthorized { code: String, message: String },
    #[error("{message}")]
    Forbidden { code: String, message: String },
    #[error("{message}")]
    NotFound { code: String, message: String },
    #[error("{message}")]
    Conflict { code: String, message: String },
    #[error("{message}")]
    Internal { code: String, message: String },
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            code: "bad_request".into(),
            message: message.into(),
        }
    }
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized {
            code: "unauthorized".into(),
            message: message.into(),
        }
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden {
            code: "forbidden".into(),
            message: message.into(),
        }
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            code: "not_found".into(),
            message: message.into(),
        }
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict {
            code: "conflict".into(),
            message: message.into(),
        }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            code: "internal_error".into(),
            message: message.into(),
        }
    }
    pub fn code_with(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::BadRequest {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn code(&self) -> &str {
        match self {
            Self::BadRequest { code, .. }
            | Self::Unauthorized { code, .. }
            | Self::Forbidden { code, .. }
            | Self::NotFound { code, .. }
            | Self::Conflict { code, .. }
            | Self::Internal { code, .. } => code,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::BadRequest { message, .. }
            | Self::Unauthorized { message, .. }
            | Self::Forbidden { message, .. }
            | Self::NotFound { message, .. }
            | Self::Conflict { message, .. }
            | Self::Internal { message, .. } => message,
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            Self::BadRequest { .. } => 400,
            Self::Unauthorized { .. } => 401,
            Self::Forbidden { .. } => 403,
            Self::NotFound { .. } => 404,
            Self::Conflict { .. } => 409,
            Self::Internal { .. } => 500,
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", self.message())?;
        state.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::internal(format!("数据库错误: {value}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::internal(format!("JSON 解析错误: {value}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::internal(format!("IO 错误: {value}"))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        Self::internal(format!("HTTP 请求错误: {value}"))
    }
}
