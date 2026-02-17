use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;
use tokio_postgres::types::{self, ToSql};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum SqlArg {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    Binary(Vec<u8>),
    Json(Value),
    Text(String),
    Timestamp(DateTime<Utc>),
    Date(NaiveDate),
    Uuid(Uuid),
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
            SqlArg::Date(v) => v,
            SqlArg::Uuid(v) => v,
        }
    }
}
