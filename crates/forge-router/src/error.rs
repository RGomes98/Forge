use std::fmt::Debug;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RouterError {
    #[error("{0}: duplicate route")]
    DuplicateRoute(String),
}
