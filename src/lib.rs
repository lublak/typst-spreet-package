use std::{collections::HashMap, io::Cursor};

use calamine::{Data, Reader};
use chrono::{Datelike, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};
use spreadsheet_ods::OdsOptions;
use umya_spreadsheet::reader;
use wasm_minimal_protocol::*;

initiate_protocol!();

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum SheetValue {
    Null,
    Bool(bool),
    //Int(i64),
    Float(f64),
    String(String),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum ExtendedSheetValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum SheetIdent {
    Index(usize),
    Name(String),
}

//#[derive(Serialize, Deserialize)]
//#[serde(untagged)]
//pub(crate) enum SheetDecodeRange {
//    TableName(String),
//    RowColumns(),
//}

//#[derive(Serialize, Deserialize)]
//struct SheetDecodeIdentAndRange {
//    ident: SheetIdent,
//    range: SheetDecodeRange,
//}

//#[derive(Serialize, Deserialize)]
//#[serde(untagged)]
//pub(crate) enum SheetDecode {
//    OnlySheetIdents(Vec<SheetIdent>),
//    //    IdentAndRange(Vec<SheetDecodeIdentAndRange>), todo!
//}

#[derive(Serialize, Deserialize)]
struct DateTime {
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

impl DateTime {
    pub fn parse_from_str(s: &str, fmt: &str) -> Option<DateTime> {
        NaiveDateTime::parse_from_str(s, fmt).map_or(None, |d| Some(DateTime::from_chrono(d)))
    }

    pub fn from_chrono(dt: NaiveDateTime) -> DateTime {
        DateTime {
            year: dt.year() as u32,
            month: dt.month(),
            day: dt.day(),
            hour: dt.hour(),
            minute: dt.minute(),
            second: dt.second(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct WorkBookInfos {
    title: String,
    subject: String,
    description: String,
    keywords: String,
    creator: String,
    created: Option<DateTime>,
    modified: Option<DateTime>,
    sheets: Vec<WorkSheetInfos>,
}

#[derive(Serialize, Deserialize)]
struct WorkSheetInfos {
    name: String,
    rows: Vec<RowInfos>,
    cols: Vec<ColInfos>,
    cells: Vec<CellInfos>,
}

#[derive(Serialize, Deserialize)]
struct RowInfos {
    height: String,
    hidden: bool,
}

#[derive(Serialize, Deserialize)]
struct ColInfos {
    width: String,
    hidden: bool,
}

#[derive(Serialize, Deserialize)]
struct CellInfos {
    x: u32,
    y: u32,
    col_span: u32,
    row_span: u32,
    value: SheetValue,
}

#[derive(Serialize, Deserialize)]
struct FontStyle {
    bold: bool,
    italic: bool,
    size: f64,
    color: Option<String>,
    underline: bool,
    strike: bool,
}

fn get_sheet_data(sd: calamine::Range<Data>) -> Vec<Vec<SheetValue>> {
    sd.rows()
        .map(|row| {
            row.iter()
                .map(|col| match *col {
                    Data::Int(value) => SheetValue::Float(value as f64), // if i looked correctly, there is no "int" parsing
                    Data::Float(value) => SheetValue::Float(value),
                    Data::String(ref value) => SheetValue::String(value.to_owned()),
                    Data::Bool(value) => SheetValue::Bool(value),
                    Data::DateTime(value) => SheetValue::Float(value.as_f64()),
                    Data::DateTimeIso(ref value) => SheetValue::String(value.to_owned()),
                    Data::DurationIso(ref value) => SheetValue::String(value.to_owned()),
                    Data::Error(ref value) => SheetValue::String(value.to_string()),
                    Data::Empty => SheetValue::Null,
                })
                .collect()
        })
        .collect()
}

fn get_sheet_infos_xls(s: &umya_spreadsheet::Worksheet) -> WorkSheetInfos {
    //ws.get_cell_collection_sorted().iter().map(|c| c.)
    //ws.get_merge_cells()[0].get_coordinate_start_col()
    return WorkSheetInfos {
        name: s.get_name().to_string(),
        rows: s
            .get_row_dimensions()
            .iter()
            .map(|r| RowInfos {
                height: format!("{}pt", r.get_height()),
                hidden: *r.get_hidden(),
            })
            .collect(),
        cols: s
            .get_column_dimensions()
            .iter()
            .map(|r| ColInfos {
                width: format!("{}pt", r.get_width()),
                hidden: *r.get_hidden(),
            })
            .collect(),
        cells: s
            .get_cell_collection()
            .iter()
            .map(|c| {
                let coord = c.get_coordinate();
                CellInfos {
                    x: *coord.get_col_num(),
                    y: *coord.get_row_num(),
                    value: match c.get_raw_value() {
                        umya_spreadsheet::CellRawValue::String(value) => {
                            SheetValue::String(value.to_string())
                        }
                        umya_spreadsheet::CellRawValue::RichText(rich_text) => {
                            SheetValue::String(rich_text.get_text().to_string())
                        }
                        umya_spreadsheet::CellRawValue::Lazy(_) => todo!(),
                        umya_spreadsheet::CellRawValue::Numeric(value) => SheetValue::Float(*value),
                        umya_spreadsheet::CellRawValue::Bool(value) => SheetValue::Bool(*value),
                        umya_spreadsheet::CellRawValue::Error(cell_error_type) => {
                            SheetValue::String(cell_error_type.to_string())
                        }
                        umya_spreadsheet::CellRawValue::Empty => SheetValue::Null,
                    },
                    col_span: 1,
                    row_span: 1,
                }
            })
            .collect(),
    };
}

fn get_sheet_infos_ods(s: &spreadsheet_ods::Sheet) -> WorkSheetInfos {
    return WorkSheetInfos {
        name: s.name().to_string(),
        rows: (0..s.row_header_max())
            .map(|r| RowInfos {
                height: s.row_height(r).to_string(),
                hidden: match s.row_visible(r) {
                    spreadsheet_ods::sheet::Visibility::Visible => false,
                    spreadsheet_ods::sheet::Visibility::Collapsed => true,
                    spreadsheet_ods::sheet::Visibility::Filtered => true,
                },
            })
            .collect(),
        cols: (0..s.col_header_max())
            .map(|r| ColInfos {
                width: s.col_width(r).to_string(),
                hidden: match s.row_visible(r) {
                    spreadsheet_ods::sheet::Visibility::Visible => false,
                    spreadsheet_ods::sheet::Visibility::Collapsed => true,
                    spreadsheet_ods::sheet::Visibility::Filtered => true,
                },
            })
            .collect(),
        cells: s
            .iter()
            .map(|c| CellInfos {
                x: c.0 .0,
                y: c.0 .1,
                col_span: c.1.col_span(),
                row_span: c.1.row_span(),
                value: match c.1.value {
                    spreadsheet_ods::Value::Empty => SheetValue::Null,
                    spreadsheet_ods::Value::Boolean(value) => SheetValue::Bool(*value),
                    spreadsheet_ods::Value::Number(value) => SheetValue::Float(*value),
                    spreadsheet_ods::Value::Percentage(value) => SheetValue::Float(*value),
                    spreadsheet_ods::Value::Currency(_, _) => todo!(),
                    spreadsheet_ods::Value::Text(value) => SheetValue::String(value.clone()),
                    spreadsheet_ods::Value::TextXml(xml_tags) => {
                        let mut buf = String::new();
                        for t in xml_tags {
                            if !buf.is_empty() {
                                buf.push('\n');
                            }
                            t.extract_text(&mut buf);
                        }
                        SheetValue::String(buf)
                    }
                    spreadsheet_ods::Value::DateTime(naive_date_time) => {
                        SheetValue::String(naive_date_time.to_string())
                    }
                    spreadsheet_ods::Value::TimeDuration(time_delta) => {
                        SheetValue::String(time_delta.to_string())
                    }
                },
            })
            .collect(),
    };
}

#[wasm_func]
fn decode(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut workbook = calamine::open_workbook_auto_from_rs(Cursor::new(data))
        .map_err(|e| format!("failed to deserialize data as workbook: {}", e.to_string()))?;

    let result: IndexMap<String, _> = workbook
        .worksheets()
        .into_iter()
        .map(|ws| (ws.0, get_sheet_data(ws.1)))
        .collect();

    let mut buffer = vec![];
    _ = ciborium::ser::into_writer(&result, &mut buffer)
        .map_err(|e| format!("failed to serialize results: {}", e.to_string()))?;
    Ok(buffer)
}

#[wasm_func]
fn decode_sheet_by_indexes(data: &[u8], indexes: &[u8]) -> Result<Vec<u8>, String> {
    let mut workbook = calamine::open_workbook_auto_from_rs(Cursor::new(data))
        .map_err(|e| format!("failed to deserialize data as workbook: {}", e.to_string()))?;
    let indexes: Vec<SheetIdent> = ciborium::from_reader(indexes)
        .map_err(|e| format!("failed to deserialize options: {}", e.to_string()))?;

    let names = workbook.sheet_names();

    let result: Result<IndexMap<String, Vec<Vec<SheetValue>>>, String> = indexes
        .iter()
        .map(|i| match i {
            SheetIdent::Index(index) => {
                let name = names
                    .get(*index)
                    .ok_or(format!("index {} not found", index))?
                    .to_string();
                let ws = workbook.worksheet_range(&name).map_err(|e| e.to_string())?;
                Ok((name.clone(), get_sheet_data(ws)))
            }
            SheetIdent::Name(name) => Ok((
                name.clone(),
                get_sheet_data(workbook.worksheet_range(&name).map_err(|e| e.to_string())?),
            )),
        })
        .collect();

    let result = result?;

    let mut buffer = vec![];
    _ = ciborium::ser::into_writer(&result, &mut buffer)
        .map_err(|e| format!("failed to serialize results: {}", e.to_string()))?;
    Ok(buffer)
}

#[wasm_func]
fn decode_full(data: &[u8]) -> Result<Vec<u8>, String> {
    let info: WorkBookInfos;
    if infer::doc::is_xls(data) {
        let workbook = reader::xlsx::read_reader(Cursor::new(data), true).unwrap();
        let props = workbook.get_properties();

        let sheets = workbook
            .get_sheet_collection()
            .iter()
            .map(|s| get_sheet_infos_xls(s))
            .collect();

        info = WorkBookInfos {
            title: props.get_title().to_string(),
            subject: props.get_subject().to_string(),
            description: props.get_description().to_string(),
            keywords: props.get_keywords().to_string(),
            creator: props.get_creator().to_string(),
            created: DateTime::parse_from_str(props.get_created(), "%Y-%m-%dT%H:%M:%SZ"),
            modified: DateTime::parse_from_str(props.get_modified(), "%Y-%m-%dT%H:%M:%SZ"),
            sheets: sheets,
        }
    } else if infer::odf::is_ods(data) {
        let workbook = OdsOptions::default().read_ods(Cursor::new(data)).unwrap();
        let props = workbook.metadata();

        let sheets = workbook
            .iter_sheets()
            .map(|s| get_sheet_infos_ods(s))
            .collect();

        info = WorkBookInfos {
            title: props.title.clone(),
            subject: props.subject.clone(),
            description: props.description.clone(),
            keywords: props.keyword.clone(),
            creator: props.creator.clone(),
            created: props.creation_date.map(|dt| DateTime::from_chrono(dt)),
            modified: props.date.map(|dt| DateTime::from_chrono(dt)),
            sheets: sheets,
        }
    } else {
        return Err("invalid data (no xlsx, ods)".to_string());
    }
    let mut buffer = vec![];
    _ = ciborium::ser::into_writer(&info, &mut buffer)
        .map_err(|e| format!("failed to serialize results: {}", e.to_string()))?;
    Ok(buffer)
}

#[wasm_func]
fn decode_full_by_indexes(data: &[u8], indexes: &[u8]) -> Result<Vec<u8>, String> {
    let indexes: Vec<SheetIdent> = ciborium::from_reader(indexes)
        .map_err(|e| format!("failed to deserialize options: {}", e.to_string()))?;

    let info: WorkBookInfos;
    if infer::doc::is_xls(data) {
        let workbook = reader::xlsx::read_reader(Cursor::new(data), true).unwrap();
        let props = workbook.get_properties();

        let sheets: Result<Vec<WorkSheetInfos>, String> = indexes
            .iter()
            .map(|i| match i {
                SheetIdent::Index(index) => Ok(get_sheet_infos_xls(
                    workbook
                        .get_sheet(index)
                        .ok_or(format!("index {} not found", index))?,
                )),
                SheetIdent::Name(name) => Ok(get_sheet_infos_xls(
                    workbook
                        .get_sheet_by_name(name)
                        .ok_or(format!("sheet with name {} not found", name))?,
                )),
            })
            .collect();

        let sheets = sheets?;

        info = WorkBookInfos {
            title: props.get_title().to_string(),
            subject: props.get_subject().to_string(),
            description: props.get_description().to_string(),
            keywords: props.get_keywords().to_string(),
            creator: props.get_creator().to_string(),
            created: DateTime::parse_from_str(props.get_created(), "%Y-%m-%dT%H:%M:%SZ"),
            modified: DateTime::parse_from_str(props.get_modified(), "%Y-%m-%dT%H:%M:%SZ"),
            sheets: sheets,
        }
    } else if infer::odf::is_ods(data) {
        let workbook = OdsOptions::default().read_ods(Cursor::new(data)).unwrap();
        let props = workbook.metadata();

        let sheets: Result<Vec<WorkSheetInfos>, String> = indexes
            .iter()
            .map(|i| match i {
                SheetIdent::Index(index) => {
                    if index >= &workbook.num_sheets() {
                        return Err(format!("index {} not found", index));
                    }

                    Ok(get_sheet_infos_ods(workbook.sheet(*index)))
                }
                SheetIdent::Name(name) => Ok(get_sheet_infos_ods(
                    workbook.sheet(
                        workbook
                            .sheet_idx(name)
                            .ok_or(format!("sheet with name {} not found", name))?,
                    ),
                )),
            })
            .collect();

        let sheets = sheets?;

        info = WorkBookInfos {
            title: props.title.clone(),
            subject: props.subject.clone(),
            description: props.description.clone(),
            keywords: props.keyword.clone(),
            creator: props.creator.clone(),
            created: props.creation_date.map(|dt| DateTime::from_chrono(dt)),
            modified: props.date.map(|dt| DateTime::from_chrono(dt)),
            sheets: sheets,
        }
    } else {
        return Err("invalid data (no xlsx, ods)".to_string());
    }
    let mut buffer = vec![];
    _ = ciborium::ser::into_writer(&info, &mut buffer)
        .map_err(|e| format!("failed to serialize results: {}", e.to_string()))?;
    Ok(buffer)
}
