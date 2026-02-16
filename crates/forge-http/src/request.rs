use std::borrow::Cow;
use std::collections::HashMap;
use std::str::{FromStr, Lines, SplitWhitespace};

use super::HttpError;
use super::HttpMethod;
use super::HttpStatus;

type RequestLine<'a> = (&'a str, &'a str, HttpMethod);
pub type Headers<'a> = HashMap<Cow<'a, str>, Cow<'a, str>>;
pub type Params<'a> = HashMap<&'a str, &'a str>;

const HEADERS_SEPARATOR: char = ':';

#[derive(Debug)]
pub struct Request<'a> {
    pub method: HttpMethod,
    pub path: &'a str,
    pub version: &'a str,
    pub headers: Headers<'a>,
    pub params: Params<'a>,
}

impl<'a> Request<'a> {
    pub fn new(raw_request: &'a str) -> Result<Self, HttpError> {
        let mut lines: Lines = raw_request.lines();

        let request_lines: &str = lines
            .next()
            .ok_or_else(|| HttpError::new(HttpStatus::BadRequest, "Request line is empty or missing"))?;

        let (path, version, method): RequestLine = Self::parse_request_line(request_lines)?;
        let headers: Headers = Self::parse_headers(lines)?;

        Ok(Self {
            headers,
            path,
            version,
            method,
            params: HashMap::new(),
        })
    }

    pub fn set_params(&mut self, raw_params: Vec<(&'a str, &'a str)>) {
        self.params.extend(raw_params);
    }

    fn parse_headers(raw_headers: Lines) -> Result<Headers, HttpError> {
        raw_headers
            .take_while(|line: &&str| !line.trim().is_empty())
            .map(|header: &str| {
                let values: (&str, &str) = header.split_once(HEADERS_SEPARATOR).ok_or_else(|| {
                    HttpError::new(HttpStatus::BadRequest, format!("Invalid header format: \"{header}\""))
                })?;

                let key: &str = values.0.trim();
                let value: &str = values.1.trim();

                let key_cow: Cow<str> = if key.as_bytes().iter().any(|byte: &u8| byte.is_ascii_uppercase()) {
                    Cow::Owned(key.to_ascii_lowercase())
                } else {
                    Cow::Borrowed(key)
                };

                Ok((key_cow, Cow::Borrowed(value)))
            })
            .collect::<Result<Headers, HttpError>>()
    }

    fn parse_request_line(raw_request_line: &str) -> Result<RequestLine<'_>, HttpError> {
        let mut parts: SplitWhitespace = raw_request_line.split_whitespace();

        let method_str: &str = parts
            .next()
            .ok_or_else(|| HttpError::new(HttpStatus::BadRequest, "Request line missing HTTP Method"))?;

        let path: &str = parts
            .next()
            .ok_or_else(|| HttpError::new(HttpStatus::BadRequest, "Request line missing URI Path"))?;

        let version: &str = parts
            .next()
            .ok_or_else(|| HttpError::new(HttpStatus::BadRequest, "Request line missing HTTP Version"))?;

        let method: HttpMethod = HttpMethod::from_str(method_str)?;
        Ok((path, version, method))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_simple_request() {
        let raw: &str = "GET /index.html HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let req: Request = Request::new(raw).expect("Should parse valid request");

        assert_eq!(req.method, HttpMethod::GET);
        assert_eq!(req.path, "/index.html");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.get("host").map(|v| v.as_ref()), Some("localhost"));
    }

    #[test]
    fn test_parse_headers_case_insensitivity() {
        let raw: &str = "POST /submit HTTP/1.1\r\nCONTENT-TYPE: application/json\r\nX-Custom-Header: value\r\n\r\n";
        let req: Request = Request::new(raw).expect("Should parse headers");

        assert!(req.headers.contains_key("content-type"));
        assert!(req.headers.contains_key("x-custom-header"));

        match req
            .headers
            .keys()
            .find(|k: &&Cow<str>| k.as_ref() == "content-type")
            .unwrap()
        {
            Cow::Owned(_) => {}
            _ => panic!("Expected Owned Cow for converted lowercase key"),
        }
    }

    #[test]
    fn test_parse_headers_trim_whitespace() {
        let raw: &str = "GET / HTTP/1.1\r\nKey:    value with spaces    \r\n\r\n";
        let req: Request = Request::new(raw).unwrap();

        assert_eq!(req.headers.get("key").map(|v| v.as_ref()), Some("value with spaces"));
    }

    #[test]
    fn test_request_empty_string() {
        let raw: &str = "";
        let result: Result<Request, HttpError> = Request::new(raw);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, HttpStatus::BadRequest);
    }

    #[test]
    fn test_request_invalid_method() {
        let raw: &str = "INVALIDMETHOD /path HTTP/1.1\r\n\r\n";
        let result: Result<Request, HttpError> = Request::new(raw);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, HttpStatus::BadRequest);
    }

    #[test]
    fn test_request_missing_version() {
        let raw: &str = "GET /path\r\n\r\n";
        let result: Result<Request, HttpError> = Request::new(raw);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, HttpStatus::BadRequest);
    }

    #[test]
    fn test_header_missing_colon() {
        let raw: &str = "GET / HTTP/1.1\r\nInvalidHeader\r\n\r\n";
        let result: Result<Request, HttpError> = Request::new(raw);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, HttpStatus::BadRequest);
    }

    #[test]
    fn test_set_params() {
        let raw: &str = "GET /store/123 HTTP/1.1\r\n\r\n";
        let mut req: Request = Request::new(raw).unwrap();

        assert!(req.params.is_empty());

        let new_params: Vec<(&str, &str)> = vec![("store_id", "123"), ("filter", "active")];
        req.set_params(new_params);

        assert_eq!(req.params.len(), 2);
        assert_eq!(req.params.get("store_id"), Some(&"123"));
        assert_eq!(req.params.get("filter"), Some(&"active"));
    }
}
