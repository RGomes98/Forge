use std::sync::Arc;

use super::DbValue;
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use tokio_postgres::{Column, Row};

#[derive(Debug)]
pub struct RowSet {
    pub columns: Arc<[Arc<str>]>,
    pub rows: Vec<Vec<DbValue>>,
}

impl RowSet {
    pub fn from_pg_rows(rows: Vec<Row>) -> Self {
        let columns: Arc<[Arc<str>]> = rows.first().map_or_else(
            || Arc::from([]),
            |row: &Row| {
                row.columns()
                    .iter()
                    .map(|column: &Column| Arc::from(column.name()))
                    .collect::<Arc<[Arc<str>]>>()
            },
        );

        Self {
            columns,
            rows: rows.iter().map(DbValue::decode_row).collect(),
        }
    }

    pub fn as_objects(&self) -> RowSetAsObjects<'_> {
        RowSetAsObjects(self)
    }
}

#[derive(Debug)]
pub struct RowSetAsObjects<'a>(pub &'a RowSet);

impl<'a> Serialize for RowSetAsObjects<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let row_set: &RowSet = self.0;
        let mut sequence: <S as Serializer>::SerializeSeq = serializer.serialize_seq(Some(row_set.rows.len()))?;

        for row in &row_set.rows {
            sequence.serialize_element(&RowAsObject {
                columns: &row_set.columns,
                row,
            })?;
        }

        sequence.end()
    }
}

#[derive(Debug)]
struct RowAsObject<'a> {
    columns: &'a [Arc<str>],
    row: &'a [DbValue],
}

impl<'a> Serialize for RowAsObject<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map: <S as Serializer>::SerializeMap = serializer.serialize_map(Some(self.columns.len()))?;

        for (col_name, value) in self.columns.iter().zip(self.row.iter()) {
            map.serialize_entry(col_name.as_ref(), value)?;
        }

        map.end()
    }
}
