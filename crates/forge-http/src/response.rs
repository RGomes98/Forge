use std::borrow::Cow;

use super::{HttpError, HttpStatus};
use serde::Serialize;
use tokio::{io::AsyncWriteExt, net::TcpStream};
use tracing::debug;

pub struct Response<'a> {
    status: HttpStatus,
    body_content: Option<Cow<'a, str>>,
}

impl<'a> Response<'a> {
    pub fn new(status: HttpStatus) -> Self {
        Self {
            status,
            body_content: None,
        }
    }

    pub fn body<T>(mut self, body: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.body_content.replace(body.into());
        self
    }

    pub fn json<T>(self, body: T) -> Self
    where
        T: Serialize,
    {
        match serde_json::to_string(&body) {
            Ok(v) => self.body(v),
            Err(e) => Response::new(HttpStatus::InternalServerError).body(format!("JSON Serialization Failed: {e}")),
        }
    }

    pub async fn send(self, stream: &mut TcpStream) -> Result<(), HttpError> {
        let body_content: Cow<str> = self.body_content.unwrap_or_default();
        let content_length: usize = body_content.len();
        let status_code: u16 = self.status.into();
        let status: HttpStatus = self.status;

        let response: String =
            format!("HTTP/1.1 {status_code} {status}\r\nContent-Length: {content_length}\r\n\r\n{body_content}");

        debug!("Sending HTTP response: {response:#?}",);

        stream
            .write_all(response.as_bytes())
            .await
            .map_err(|_| HttpError::new(HttpStatus::InternalServerError, "Failed to write response"))?;

        stream
            .flush()
            .await
            .map_err(|_| HttpError::new(HttpStatus::InternalServerError, "Failed to flush stream"))?;

        Ok(())
    }
}

pub trait IntoResponse<'a> {
    fn into_response(self) -> Response<'a>;
}

impl<'a> IntoResponse<'a> for Response<'a> {
    fn into_response(self) -> Response<'a> {
        self
    }
}

impl<'a> From<HttpError> for Response<'a> {
    fn from(e: HttpError) -> Self {
        Response::new(e.status).body(e.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_into_response() {
        let response: Response = Response::new(HttpStatus::Ok).body("TEXT");
        let result: Response = response.into_response();

        assert_eq!(result.status, HttpStatus::Ok);
        assert_eq!(result.body_content.unwrap(), "TEXT");
    }

    #[test]
    fn test_http_error_conversion_via_into() {
        let error: HttpError = HttpError::new(HttpStatus::NotFound, "NOT_FOUND");
        let response: Response = error.into();

        assert_eq!(response.status, HttpStatus::NotFound);
        assert_eq!(response.body_content.unwrap(), "NOT_FOUND");
    }

    #[test]
    fn test_json_response_success() {
        let user: serde_json::Value = serde_json::json!({ "name": "John Doe", "age": 18 });
        let response: Response = Response::new(HttpStatus::Ok).json(&user);

        assert_eq!(response.status, HttpStatus::Ok);
        assert_eq!(response.body_content.unwrap(), r#"{"age":18,"name":"John Doe"}"#);
    }

    #[test]
    fn test_handler_returning_only_response() {
        fn mock_success_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok).body("SUCCESS")
        }

        fn mock_error_handler_converted() -> Response<'static> {
            HttpError::new(HttpStatus::Unauthorized, "UNAUTHORIZED").into()
        }

        let success: Response = mock_success_handler();
        assert_eq!(success.status, HttpStatus::Ok);
        assert_eq!(success.body_content.unwrap(), "SUCCESS");

        let error_response: Response = mock_error_handler_converted();
        assert_eq!(error_response.status, HttpStatus::Unauthorized);
        assert_eq!(error_response.body_content.unwrap(), "UNAUTHORIZED");
    }
}
