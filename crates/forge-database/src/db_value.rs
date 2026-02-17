use super::decode;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use serde::ser::{Serialize, Serializer};
use serde_json::Value;
use tokio_postgres::{Row, types::Type};
use uuid::Uuid;

#[derive(Debug)]
pub enum DbValue {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    Uuid(Uuid),
    Json(Value),
    Date(NaiveDate),
    Time(NaiveTime),
    Bytes(Vec<u8>),
    String(String),
    Timestamp(NaiveDateTime),
    TimestampTz(DateTime<Utc>),
}

impl DbValue {
    pub fn decode_row(row: &Row) -> Vec<DbValue> {
        row.columns()
            .iter()
            .enumerate()
            .map(|(idx, col)| Self::decode_cell(row, idx, col.type_()))
            .collect()
    }

    fn decode_cell(row: &Row, idx: usize, ty: &Type) -> Self {
        let ctx: (&Row, usize) = (row, idx);

        match *ty {
            Type::BOOL => decode!(ctx, bool => DbValue::Bool),
            Type::INT2 => decode!(ctx, i16 => |v| DbValue::I64(i64::from(v))),
            Type::INT4 => decode!(ctx, i32 => |v| DbValue::I64(i64::from(v))),
            Type::INT8 => decode!(ctx, i64 => DbValue::I64),
            Type::OID => decode!(ctx, u32 => |v| DbValue::U64(u64::from(v))),
            Type::FLOAT4 => decode!(ctx, f32 => |v| DbValue::F64(f64::from(v))),
            Type::FLOAT8 => decode!(ctx, f64 => DbValue::F64),
            Type::UUID => decode!(ctx, Uuid => DbValue::Uuid),
            Type::JSON | Type::JSONB => decode!(ctx, Value => DbValue::Json),
            Type::BYTEA => decode!(ctx, Vec<u8> => DbValue::Bytes),
            Type::DATE => decode!(ctx, NaiveDate => DbValue::Date),
            Type::TIME => decode!(ctx, NaiveTime =>DbValue::Time),
            Type::TIMESTAMP => decode!(ctx, NaiveDateTime => DbValue::Timestamp),
            Type::TIMESTAMPTZ => decode!(ctx, DateTime<Utc> => DbValue::TimestampTz),
            Type::NUMERIC | Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME | Type::MONEY => {
                decode!(ctx, String => DbValue::String)
            }
            _ => DbValue::Null,
        }
    }
}

impl Serialize for DbValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            DbValue::Null => serializer.serialize_unit(),
            DbValue::Bool(v) => serializer.serialize_bool(*v),
            DbValue::I64(v) => serializer.serialize_i64(*v),
            DbValue::U64(v) => serializer.serialize_u64(*v),
            DbValue::F64(v) => serializer.serialize_f64(*v),
            DbValue::String(v) => serializer.serialize_str(v),
            DbValue::Json(v) => v.serialize(serializer),
            DbValue::Uuid(v) => serializer.collect_str(v),
            DbValue::Date(v) => serializer.serialize_str(&v.to_string()),
            DbValue::Time(v) => serializer.serialize_str(&v.to_string()),
            DbValue::Timestamp(v) => serializer.serialize_str(&v.to_string()),
            DbValue::TimestampTz(v) => serializer.serialize_str(&v.to_rfc3339()),
            DbValue::Bytes(v) => serializer.serialize_bytes(v),
        }
    }
}
