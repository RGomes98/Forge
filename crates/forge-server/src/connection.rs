use std::io::Error;
use std::io::ErrorKind;
use std::str::{self, Utf8Error};
use std::sync::Arc;

use super::ListenerError;
use forge_http::{HttpError, HttpStatus, Request, Response};
use forge_router::{BoxedHandler, Router};
use forge_utils::PathMatch;
use monoio::{io::AsyncReadRent, net::TcpStream};

pub struct Connection<T> {
    pub stream: TcpStream,
    pub state: Option<Arc<T>>,
    pub router: Arc<Router<T>>,
}

impl<T> Connection<T>
where
    T: Send + Sync + 'static,
{
    pub async fn process_request(&mut self, buffer: Vec<u8>) -> Result<Vec<u8>, ListenerError> {
        let (bytes_read, buffer): (usize, Vec<u8>) = self.read_request_bytes(buffer).await?;
        let raw_bytes: &[u8] = &buffer[..bytes_read];

        let raw_request: &str = str::from_utf8(raw_bytes)
            .map_err(|e: Utf8Error| HttpError::new(HttpStatus::BadRequest, format!("Invalid UTF-8 sequence: {e}")))?;

        let mut request: Request = Request::new(raw_request)?;

        let route: PathMatch<BoxedHandler<T>> = self
            .router
            .get_route(request.path, &request.method)
            .ok_or_else(|| HttpError::new(HttpStatus::NotFound, "The requested resource could not be found"))?;

        request.set_params(route.params);
        let response: Response = route.value.call(request, self.state.clone()).await;
        response.send(&mut self.stream).await?;

        Ok(buffer)
    }

    async fn read_request_bytes(&mut self, buffer: Vec<u8>) -> Result<(usize, Vec<u8>), ListenerError> {
        let (read_result, buffer): (Result<usize, Error>, Vec<u8>) = self.stream.read(buffer).await;

        let bytes: usize = read_result.map_err(|e: Error| match e.kind() {
            ErrorKind::ConnectionReset | ErrorKind::BrokenPipe => ListenerError::ConnectionClosed,
            _ => HttpError::new(HttpStatus::InternalServerError, "Failed to read data from stream").into(),
        })?;

        if bytes == 0 {
            return Err(ListenerError::ConnectionClosed);
        }

        Ok((bytes, buffer))
    }
}
