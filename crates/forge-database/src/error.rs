use std::{fmt::Debug, io};

use super::database::DbCommand;
use thiserror::Error;
use tokio::sync::{mpsc::error::SendError, oneshot::error::RecvError};
use tokio_postgres::error::DbError;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("database connection pool is closed or shutting down: {0}")]
    PoolClosed(#[from] SendError<DbCommand>),

    #[error("database worker terminated without responding")]
    NoResponse(#[from] RecvError),

    #[error("database transport layer error: {0}")]
    Transport(#[from] io::Error),

    #[error("{}", .0.as_db_error().map(|db_err: &DbError| db_err.to_string()).unwrap_or_else(|| .0.to_string()))]
    Postgres(#[from] tokio_postgres::Error),
}
