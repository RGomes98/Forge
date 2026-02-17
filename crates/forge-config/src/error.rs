use std::env::VarError;
use std::fmt::Debug;
use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Environment variable is missing or invalid: {0}")]
    MissingOrInvalid(#[from] VarError),

    #[error("Failed to parse string value: {0}")]
    StringParse(#[source] Box<dyn std::error::Error>),

    #[error("Failed to parse TOML content: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("I/O Error: {0}")]
    Io(#[from] io::Error),
}
