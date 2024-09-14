use std::{collections::HashMap, io::Cursor};

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use serde::{Deserialize, Serialize};
use wasm_minimal_protocol::*;

initiate_protocol!();

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum ExcelValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

#[wasm_func]
fn decode(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(data))
        .map_err(|e| format!("failed to deserialize data as workbook: {}", e.to_string()))?;
    let result: HashMap<String, Vec<Vec<ExcelValue>>> = workbook
        .worksheets()
        .into_iter()
        .map(|ws| {
            (
                ws.0,
                ws.1.rows()
                    .map(|row| {
                        row.iter()
                            .map(|col| match *col {
                                Data::Int(value) => ExcelValue::Int(value),
                                Data::Float(value) => ExcelValue::Float(value),
                                Data::String(ref value) => ExcelValue::String(value.to_owned()),
                                Data::Bool(value) => ExcelValue::Bool(value),
                                Data::DateTime(value) => ExcelValue::Float(value.as_f64()),
                                Data::DateTimeIso(ref value) => {
                                    ExcelValue::String(value.to_owned())
                                }
                                Data::DurationIso(ref value) => {
                                    ExcelValue::String(value.to_owned())
                                }
                                Data::Error(ref value) => ExcelValue::String(value.to_string()),
                                Data::Empty => ExcelValue::Null,
                            })
                            .collect()
                    })
                    .collect(),
            )
        })
        .collect();

    let mut buffer = vec![];
    _ = ciborium::ser::into_writer(&result, &mut buffer)
        .map_err(|e| format!("failed to serialize results: {}", e.to_string()))?;
    Ok(buffer)
}
