use std::io::Error;

use super::HttpStatus;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("[{}] {}: {}", u16::from(self.status), status, message)]
pub struct HttpError {
    pub status: HttpStatus,
    pub message: String,
}

impl HttpError {
    pub fn new(status: HttpStatus, msg: impl Into<String>) -> Self {
        HttpError {
            status,
            message: msg.into(),
        }
    }
}

impl From<Error> for HttpError {
    fn from(e: Error) -> Self {
        HttpError::new(HttpStatus::InternalServerError, e.to_string())
    }
}
