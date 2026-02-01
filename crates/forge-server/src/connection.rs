use std::io::Error;
use std::str::Utf8Error;
use std::sync::Arc;
use std::{io::ErrorKind, net::SocketAddr};

use super::ListenerError;
use forge_http::{HttpError, HttpStatus, Request, Response};
use forge_router::{Handler, Router};
use forge_utils::PathMatch;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tracing::{debug, warn};

const BUFFER_SIZE: usize = 4096;

pub struct Connection {
    pub router: Arc<Router>,
    pub stream: TcpStream,
}

impl Connection {
    pub async fn process_request(&mut self) -> Result<(), ListenerError> {
        let peer_addr: Option<SocketAddr> = self.stream.peer_addr().ok();
        debug!("Processing connection from: {peer_addr:?}");

        let mut buffer: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE]; // TODO: Dynamic Buffer
        let bytes_read: usize = self.read_request_bytes(&mut buffer).await?;
        let raw_bytes: &[u8] = &buffer[..bytes_read];

        let raw_request: &str = str::from_utf8(raw_bytes).map_err(|e: Utf8Error| {
            warn!("Invalid UTF-8 sequence from {peer_addr:?}: {e}");
            HttpError::new(HttpStatus::BadRequest, format!("Invalid UTF-8 sequence: {e}"))
        })?;

        let mut request: Request = Request::new(raw_request).inspect_err(|e: &HttpError| {
            warn!("Failed to parse request from {peer_addr:?}: {e}");
        })?;

        let route: PathMatch<Handler> = self.router.get_route(request.path, &request.method).ok_or_else(|| {
            warn!("404 Not Found: [{}] \"{}\"", request.method, request.path);
            HttpError::new(HttpStatus::NotFound, "The requested resource could not be found")
        })?;

        request.set_params(route.params);
        let response: Response = (route.value)(request).await;
        response.send(&mut self.stream).await?;

        debug!("Request finished successfully");
        Ok(())
    }

    async fn read_request_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, ListenerError> {
        let bytes: usize = self.stream.read(buffer).await.map_err(|e: Error| match e.kind() {
            ErrorKind::ConnectionReset | ErrorKind::BrokenPipe => ListenerError::ConnectionClosed,
            _ => HttpError::new(HttpStatus::InternalServerError, "Failed to read data from stream").into(),
        })?;

        if bytes == 0 {
            return Err(ListenerError::ConnectionClosed);
        }

        Ok(bytes)
    }
}
