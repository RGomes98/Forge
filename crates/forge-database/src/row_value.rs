use super::decode;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use serde::ser::{Serialize, Serializer};
use serde_json::Value;
use tokio_postgres::{Row, types::Type};
use uuid::Uuid;

#[derive(Debug)]
pub enum RowValue {
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

impl RowValue {
    pub fn decode_row(row: &Row) -> Vec<RowValue> {
        row.columns()
            .iter()
            .enumerate()
            .map(|(idx, col)| Self::decode_cell(row, idx, col.type_()))
            .collect()
    }

    fn decode_cell(row: &Row, idx: usize, ty: &Type) -> Self {
        let ctx: (&Row, usize) = (row, idx);

        match *ty {
            Type::BOOL => decode!(ctx, bool => RowValue::Bool),
            Type::INT2 => decode!(ctx, i16 => |v| RowValue::I64(i64::from(v))),
            Type::INT4 => decode!(ctx, i32 => |v| RowValue::I64(i64::from(v))),
            Type::INT8 => decode!(ctx, i64 => RowValue::I64),
            Type::OID => decode!(ctx, u32 => |v| RowValue::U64(u64::from(v))),
            Type::FLOAT4 => decode!(ctx, f32 => |v| RowValue::F64(f64::from(v))),
            Type::FLOAT8 => decode!(ctx, f64 => RowValue::F64),
            Type::UUID => decode!(ctx, Uuid => RowValue::Uuid),
            Type::JSON | Type::JSONB => decode!(ctx, Value => RowValue::Json),
            Type::BYTEA => decode!(ctx, Vec<u8> => RowValue::Bytes),
            Type::DATE => decode!(ctx, NaiveDate => RowValue::Date),
            Type::TIME => decode!(ctx, NaiveTime =>RowValue::Time),
            Type::TIMESTAMP => decode!(ctx, NaiveDateTime => RowValue::Timestamp),
            Type::TIMESTAMPTZ => decode!(ctx, DateTime<Utc> => RowValue::TimestampTz),
            Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME | Type::MONEY => {
                decode!(ctx, String => RowValue::String)
            }
            _ => RowValue::Null,
        }
    }
}

impl Serialize for RowValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            RowValue::Null => serializer.serialize_unit(),
            RowValue::Bool(v) => serializer.serialize_bool(*v),
            RowValue::I64(v) => serializer.serialize_i64(*v),
            RowValue::U64(v) => serializer.serialize_u64(*v),
            RowValue::F64(v) => serializer.serialize_f64(*v),
            RowValue::String(v) => serializer.serialize_str(v),
            RowValue::Json(v) => v.serialize(serializer),
            RowValue::Uuid(v) => serializer.collect_str(v),
            RowValue::Date(v) => serializer.serialize_str(&v.to_string()),
            RowValue::Time(v) => serializer.serialize_str(&v.to_string()),
            RowValue::Timestamp(v) => serializer.serialize_str(&v.to_string()),
            RowValue::TimestampTz(v) => serializer.serialize_str(&v.to_rfc3339()),
            RowValue::Bytes(v) => serializer.serialize_bytes(v),
        }
    }
}
