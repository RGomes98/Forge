use std::env;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use super::ConfigError;
use serde::de::DeserializeOwned;

pub struct Config;
impl Config {
    pub fn from_env<T>(key: &'static str) -> Result<T, ConfigError>
    where
        T: FromStr,
        T::Err: std::error::Error + 'static,
    {
        let value_str: String = env::var(key).map_err(|_| ConfigError::MissingOrInvalid(key.into()))?;

        let value: T = value_str
            .parse::<T>()
            .map_err(|e: <T as FromStr>::Err| ConfigError::StringParse(Box::new(e)))?;

        Ok(value)
    }

    pub fn from_file<T, P>(path: P) -> Result<T, ConfigError>
    where
        T: DeserializeOwned,
        P: AsRef<Path>,
    {
        let content: String = fs::read_to_string(path.as_ref())?;
        let config: T = toml::from_str(&content)?;
        Ok(config)
    }
}
