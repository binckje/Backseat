use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
pub enum AppError {
    Io(String),
    Http(String),
    Cloud(String),
    Permission(String),
    NotFound(String),
    Config(String),
    Other(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::Io(s)         => write!(f, "IO: {}", s),
            AppError::Http(s)       => write!(f, "HTTP: {}", s),
            AppError::Cloud(s)      => write!(f, "Cloud: {}", s),
            AppError::Permission(s) => write!(f, "Permission: {}", s),
            AppError::NotFound(s)   => write!(f, "Not found: {}", s),
            AppError::Config(s)     => write!(f, "Config: {}", s),
            AppError::Other(s)      => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for AppError {}

pub type Result<T> = std::result::Result<T, AppError>;

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self { AppError::Io(e.to_string()) }
}
impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self { AppError::Http(e.to_string()) }
}
impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self { AppError::Config(e.to_string()) }
}
