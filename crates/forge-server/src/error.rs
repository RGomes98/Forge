use std::{io, net::SocketAddr};

use forge_http::HttpError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ListenerError {
    #[error(transparent)]
    Http(#[from] HttpError),

    #[error("connection closed by peer")]
    ConnectionClosed,

    #[error("failed to start runtime (thread {0}): {1}")]
    Runtime(usize, io::Error),

    #[error("bind failed on {0} (thread {1}): {2}")]
    Bind(SocketAddr, usize, io::Error),

    #[error("(thread {0}) panicked: {1}")]
    ThreadPanic(usize, String),
}
