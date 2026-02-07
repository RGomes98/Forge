use std::{borrow::Cow, io::Write};

use super::{HttpError, HttpStatus};
use monoio::{io::AsyncWriteRent, io::AsyncWriteRentExt, net::TcpStream};
use serde::Serialize;

const BUFFER_SIZE: usize = 1024;

pub struct Response<'a> {
    status: HttpStatus,
    body: Option<Cow<'a, str>>,
    headers: Vec<(Cow<'a, str>, Cow<'a, str>)>,
}

impl<'a> Response<'a> {
    pub fn new(status: HttpStatus) -> Self {
        Self {
            status,
            body: None,
            headers: Vec::new(),
        }
    }

    pub fn body<T>(mut self, body: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.body.replace(body.into());
        self
    }

    pub fn header<T, K>(mut self, key: T, value: K) -> Self
    where
        T: Into<Cow<'a, str>>,
        K: Into<Cow<'a, str>>,
    {
        self.headers.push((key.into(), value.into()));
        self
    }

    pub fn text<T>(self, text: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.header("Content-Type", "text/plain").body(text)
    }

    pub fn json<T>(mut self, body: T) -> Self
    where
        T: Serialize,
    {
        match serde_json::to_string(&body) {
            Ok(v) => self.header("Content-Type", "application/json").body(v),
            Err(e) => {
                self.status = HttpStatus::InternalServerError;
                self.body.replace(format!("JSON Serialization Failed: {e}").into());
                self
            }
        }
    }

    fn write_head_to_vec(&self, buffer: &mut Vec<u8>) -> Result<(), HttpError> {
        write!(buffer, "HTTP/1.1 {} {}\r\n", u16::from(self.status), self.status)
            .map_err(|_| HttpError::new(HttpStatus::InternalServerError, "Headers too long for buffer"))?;

        for (key, value) in &self.headers {
            write!(buffer, "{key}: {value}\r\n")
                .map_err(|_| HttpError::new(HttpStatus::InternalServerError, "Headers too long for buffer"))?;
        }

        let content_length: usize = self.body.as_ref().map(|b: &Cow<str>| b.len()).unwrap_or(0);
        write!(buffer, "Content-Length: {content_length}\r\n\r\n")
            .map_err(|_| HttpError::new(HttpStatus::InternalServerError, "Headers too long for buffer"))?;

        Ok(())
    }

    pub async fn send(self, stream: &mut TcpStream) -> Result<(), HttpError> {
        let content_length: usize = self.body.as_ref().map(|b: &Cow<str>| b.len()).unwrap_or(0);

        let mut payload: Vec<u8> = Vec::with_capacity(BUFFER_SIZE + content_length);
        self.write_head_to_vec(&mut payload)?;

        if let Some(body) = &self.body {
            payload.extend_from_slice(body.as_bytes());
        }

        stream
            .write_all(payload)
            .await
            .0
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
        let response: Response = Response::new(HttpStatus::Ok).text("TEXT");
        let result: Response = response.into_response();

        assert_eq!(result.status, HttpStatus::Ok);
        assert_eq!(result.body.unwrap(), "TEXT");
    }

    #[test]
    fn test_http_error_conversion_via_into() {
        let error: HttpError = HttpError::new(HttpStatus::NotFound, "NOT_FOUND");
        let response: Response = error.into();

        assert_eq!(response.status, HttpStatus::NotFound);
        assert_eq!(response.body.unwrap(), "NOT_FOUND");
    }

    #[test]
    fn test_json_response_success() {
        let user: serde_json::Value = serde_json::json!({ "name": "John Doe", "age": 18 });
        let response: Response = Response::new(HttpStatus::Ok).json(&user);

        assert_eq!(response.status, HttpStatus::Ok);
        assert_eq!(response.body.unwrap(), r#"{"age":18,"name":"John Doe"}"#);
    }

    #[test]
    fn test_handler_returning_only_response() {
        fn mock_success_handler() -> Response<'static> {
            Response::new(HttpStatus::Ok).text("SUCCESS")
        }

        fn mock_error_handler_converted() -> Response<'static> {
            HttpError::new(HttpStatus::Unauthorized, "UNAUTHORIZED").into()
        }

        let success: Response = mock_success_handler();
        assert_eq!(success.status, HttpStatus::Ok);
        assert_eq!(success.body.unwrap(), "SUCCESS");

        let error_response: Response = mock_error_handler_converted();
        assert_eq!(error_response.status, HttpStatus::Unauthorized);
        assert_eq!(error_response.body.unwrap(), "UNAUTHORIZED");
    }
}
