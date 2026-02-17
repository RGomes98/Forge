use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio_postgres::types::{self, ToSql};

#[derive(Debug)]
pub enum SqlArg {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    Binary(Vec<u8>),
    Json(Value),
    Text(String),
    Timestamp(DateTime<Utc>),
}

impl SqlArg {
    pub fn as_sql(&self) -> &(dyn types::ToSql + Sync) {
        match self {
            SqlArg::Null => &None::<i32> as &(dyn ToSql + Sync),
            SqlArg::Bool(v) => v,
            SqlArg::Integer(v) => v,
            SqlArg::Float(v) => v,
            SqlArg::Text(v) => v,
            SqlArg::Json(v) => v,
            SqlArg::Binary(v) => v,
            SqlArg::Timestamp(v) => v,
        }
    }
}
