use std::io::Cursor;

use calamine::{Data, Reader};
use chrono::{Datelike, NaiveDateTime, Timelike};
use indexmap::IndexMap;
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
    Float(f64),
    String(String),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum SheetIdent {
    Index(usize),
    Name(String),
}

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
    style: CellStyle,
}

#[derive(Serialize, Deserialize)]
struct CellStyle {
    font: CellFontStyle,
    //horizontal_align: String,
    //vertical_align: String,
    //border: CellBorderStyle,
    //format: String,
}

//#[derive(Serialize, Deserialize)]
//struct CellBorderStyle {
//    left: String,
//    top: String,
//    right: String,
//    bottom: String,
//}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CellFontUnderline {
    Double,
    None,
    Single,
}

#[derive(Serialize, Deserialize)]
struct CellFontStyle {
    bold: bool,
    italic: bool,
    size: String,
    color: String,
    underline: CellFontUnderline,
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

fn get_cell_style_xls(cell: &umya_spreadsheet::Cell) -> CellStyle {
    let cell_style = cell.get_style();

    let font = cell_style
        .get_font()
        .map(|font| CellFontStyle {
            bold: *font.get_bold(),
            italic: *font.get_italic(),
            size: format!("{}pt", font.get_size()),
            color: format!("#{}", font.get_color().get_argb().to_lowercase()),
            underline: match font.get_font_underline().get_val() {
                umya_spreadsheet::structs::UnderlineValues::Double => CellFontUnderline::Double,
                umya_spreadsheet::structs::UnderlineValues::DoubleAccounting => {
                    CellFontUnderline::Double
                }
                umya_spreadsheet::structs::UnderlineValues::None => CellFontUnderline::None,
                umya_spreadsheet::structs::UnderlineValues::Single => CellFontUnderline::Single,
                umya_spreadsheet::structs::UnderlineValues::SingleAccounting => {
                    CellFontUnderline::Single
                }
            },
            strike: *font.get_strikethrough(),
        })
        .unwrap_or(CellFontStyle {
            bold: false,
            italic: false,
            size: "10pt".to_string(),
            color: "#000000".to_string(),
            underline: CellFontUnderline::None,
            strike: false,
        });

    CellStyle { font }
}

fn get_cell_span_xls(col: u32, row: u32, merged: &[umya_spreadsheet::Range]) -> (u32, u32, bool) {
    for merged_range in merged {
        let start_col = merged_range
            .get_coordinate_start_col()
            .map(|c| *c.get_num())
            .unwrap_or(1);
        let start_row = merged_range
            .get_coordinate_start_row()
            .map(|r| *r.get_num())
            .unwrap_or(1);
        let end_col = merged_range
            .get_coordinate_end_col()
            .map(|c| *c.get_num())
            .unwrap_or(start_col);
        let end_row = merged_range
            .get_coordinate_end_row()
            .map(|r| *r.get_num())
            .unwrap_or(start_row);

        if col >= start_col && col <= end_col && row >= start_row && row <= end_row {
            if col == start_col && row == start_row {
                let col_span = end_col.saturating_sub(start_col).saturating_add(1);
                let row_span = end_row.saturating_sub(start_row).saturating_add(1);
                return (col_span, row_span, false);
            }
            return (1, 1, true);
        }
    }
    return (1, 1, false);
}

fn get_sheet_infos_xls(s: &umya_spreadsheet::Worksheet) -> WorkSheetInfos {
    let merged = s.get_merge_cells();
    let mut row_header_max = 0;
    let mut col_header_max = 0;
    for merged_range in merged {
        let start_col = merged_range
            .get_coordinate_start_col()
            .map(|c| *c.get_num())
            .unwrap_or(1);
        let start_row = merged_range
            .get_coordinate_start_row()
            .map(|r| *r.get_num())
            .unwrap_or(1);
        let end_col = merged_range
            .get_coordinate_end_col()
            .map(|c| *c.get_num())
            .unwrap_or(start_col);
        let end_row = merged_range
            .get_coordinate_end_row()
            .map(|r| *r.get_num())
            .unwrap_or(start_row);

        if end_row > row_header_max {
            row_header_max = end_row;
        }

        if end_col > col_header_max {
            col_header_max = end_col;
        }
    }
    for c in s.get_cell_collection() {
        let coord = c.get_coordinate();
        let col = *coord.get_col_num();
        let row = *coord.get_row_num();
        if row > row_header_max {
            row_header_max = row;
        }

        if col > col_header_max {
            col_header_max = col;
        }
    }
    return WorkSheetInfos {
        name: s.get_name().to_string(),
        rows: (0..row_header_max)
            .map(|r| {
                let r = r + 1;
                s.get_row_dimension(&r).map_or_else(
                    || {
                        let def = umya_spreadsheet::Row::default();
                        RowInfos {
                            height: format!("{}pt", def.get_height()),
                            hidden: *def.get_hidden(),
                        }
                    },
                    |r| RowInfos {
                        height: format!("{}pt", r.get_height()),
                        hidden: *r.get_hidden(),
                    },
                )
            })
            .collect(),
        cols: (0..col_header_max)
            .map(|c| {
                let c = c + 1;
                s.get_column_dimension_by_number(&c).map_or_else(
                    || {
                        let def = umya_spreadsheet::Column::default();
                        ColInfos {
                            width: format!("{}pt", def.get_width()),
                            hidden: *def.get_hidden(),
                        }
                    },
                    |r| ColInfos {
                        width: format!("{}pt", r.get_width()),
                        hidden: *r.get_hidden(),
                    },
                )
            })
            .collect(),
        cells: s
            .get_cell_collection()
            .iter()
            .filter_map(|c| {
                let coord = c.get_coordinate();
                let col = *coord.get_col_num();
                let row = *coord.get_row_num();
                let (col_span, row_span, already_inside_span) = get_cell_span_xls(col, row, merged);
                if already_inside_span {
                    None
                } else {
                    Some(CellInfos {
                        x: col - 1,
                        y: row - 1,
                        value: match c.get_raw_value() {
                            umya_spreadsheet::CellRawValue::String(value) => {
                                SheetValue::String(value.to_string())
                            }
                            umya_spreadsheet::CellRawValue::RichText(rich_text) => {
                                SheetValue::String(rich_text.get_text().to_string())
                            }
                            umya_spreadsheet::CellRawValue::Lazy(_) => SheetValue::Null,
                            umya_spreadsheet::CellRawValue::Numeric(value) => {
                                SheetValue::Float(*value)
                            }
                            umya_spreadsheet::CellRawValue::Bool(value) => SheetValue::Bool(*value),
                            umya_spreadsheet::CellRawValue::Error(cell_error_type) => {
                                SheetValue::String(cell_error_type.to_string())
                            }
                            umya_spreadsheet::CellRawValue::Empty => SheetValue::Null,
                        },
                        col_span: col_span,
                        row_span: row_span,
                        style: get_cell_style_xls(c),
                    })
                }
            })
            .collect(),
    };
}

fn get_cell_span_ods(
    col: u32,
    row: u32,
    col_span: u32,
    row_span: u32,
    merged: &Vec<OdsMergedCellInfo>,
) -> (u32, u32, bool) {
    if col_span > 1 || row_span > 1 {
        return (col_span, row_span, false);
    }
    for merged_range in merged {
        let start_col = merged_range.start_col;
        let start_row = merged_range.start_row;
        let end_col = merged_range.end_col;
        let end_row = merged_range.end_row;

        if col >= start_col && col <= end_col && row >= start_row && row <= end_row {
            return (1, 1, true);
        }
    }
    return (1, 1, false);
}

fn ods_cellstyle<'a>(
    wb: &'a spreadsheet_ods::WorkBook,
    s: &spreadsheet_ods::Sheet,
    cell: &spreadsheet_ods::CellContentRef<'_>,
    col: u32,
    row: u32,
) -> Option<&'a spreadsheet_ods::CellStyle> {
    cell.style()
        .or_else(|| s.col_cellstyle(col))
        .or_else(|| s.row_cellstyle(row))
        .and_then(|style| wb.cellstyle(style))
}

fn get_cell_style_ods(
    w: &spreadsheet_ods::WorkBook,
    s: &spreadsheet_ods::Sheet,
    cell: &spreadsheet_ods::CellContentRef,
    col: u32,
    row: u32,
) -> CellStyle {
    match ods_cellstyle(w, s, cell, col, row) {
        Some(s) => {
            let textstyle = s.textstyle();
            return CellStyle {
                font: CellFontStyle {
                    bold: textstyle
                        .attr("fo:font-weight")
                        .map(|w| w == "bold")
                        .unwrap_or(false),
                    italic: textstyle
                        .attr("fo:font-style")
                        .map(|s| s == "italic")
                        .unwrap_or(false),
                    size: textstyle.attr("fo:font-size").unwrap_or("10pt").to_string(),
                    color: textstyle.attr("fo:color").unwrap_or("#000000").to_string(),
                    underline: textstyle
                        .attr("style:text-underline-style")
                        .map(|v| match v {
                            "none" => CellFontUnderline::None,
                            "solid" => textstyle
                                .attr("style:text-underline-style")
                                .map(|v| match v {
                                    "double" => CellFontUnderline::Double,
                                    _ => CellFontUnderline::Single,
                                })
                                .unwrap_or(CellFontUnderline::None),
                            _ => CellFontUnderline::None,
                        })
                        .unwrap_or(CellFontUnderline::None),
                    strike: textstyle
                        .attr("style:text-line-through-style")
                        .map(|s| !s.is_empty() && s != "none")
                        .unwrap_or(false),
                },
            };
        }
        None => CellStyle {
            font: CellFontStyle {
                bold: false,
                italic: false,
                size: "10pt".to_string(),
                color: "#hmmm".to_string(),
                underline: CellFontUnderline::None,
                strike: false,
            },
        },
    }
}

pub struct OdsMergedCellInfo {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
}

fn get_merge_cells_ods(sheet: &spreadsheet_ods::Sheet) -> Vec<OdsMergedCellInfo> {
    let mut merged_cells = Vec::new();

    for ((row, col), _) in sheet.iter() {
        let row_span = sheet.row_span(row, col);
        let col_span = sheet.col_span(row, col);

        if row_span > 1 || col_span > 1 {
            merged_cells.push(OdsMergedCellInfo {
                start_col: col,
                start_row: row,
                end_col: col + col_span,
                end_row: row + row_span,
            });
        }
    }

    merged_cells
}

fn get_sheet_infos_ods(
    w: &spreadsheet_ods::WorkBook,
    s: &spreadsheet_ods::Sheet,
) -> WorkSheetInfos {
    let merged = get_merge_cells_ods(s);
    let mut row_header_max = 0;
    let mut col_header_max = 0;
    for c in s.iter() {
        let (row, col) = c.0;
        let end_row = row + c.1.row_span();
        let end_col = col + c.1.col_span();
        if end_row > row_header_max {
            row_header_max = end_row;
        }

        if end_col > col_header_max {
            col_header_max = end_col;
        }
    }
    return WorkSheetInfos {
        name: s.name().to_string(),
        rows: (0..row_header_max)
            .map(|r| RowInfos {
                height: s.row_height(r).to_string(),
                hidden: match s.row_visible(r) {
                    spreadsheet_ods::sheet::Visibility::Visible => false,
                    spreadsheet_ods::sheet::Visibility::Collapsed => true,
                    spreadsheet_ods::sheet::Visibility::Filtered => true,
                },
            })
            .collect(),
        cols: (0..col_header_max)
            .map(|r| ColInfos {
                width: s.col_width(r).to_string(),
                hidden: match s.col_visible(r) {
                    spreadsheet_ods::sheet::Visibility::Visible => false,
                    spreadsheet_ods::sheet::Visibility::Collapsed => true,
                    spreadsheet_ods::sheet::Visibility::Filtered => true,
                },
            })
            .collect(),
        cells: s
            .iter()
            .filter_map(|c| {
                let (row, col) = c.0;
                let (col_span, row_span, already_inside_span) =
                    get_cell_span_ods(col, row, c.1.col_span(), c.1.row_span(), &merged);
                if already_inside_span {
                    None
                } else {
                    Some(CellInfos {
                        x: col,
                        y: row,
                        col_span: col_span,
                        row_span: row_span,
                        value: match c.1.value {
                            spreadsheet_ods::Value::Empty => SheetValue::Null,
                            spreadsheet_ods::Value::Boolean(value) => SheetValue::Bool(*value),
                            spreadsheet_ods::Value::Number(value) => SheetValue::Float(*value),
                            spreadsheet_ods::Value::Percentage(value) => SheetValue::Float(*value),
                            spreadsheet_ods::Value::Currency(value, _) => SheetValue::Float(*value),
                            spreadsheet_ods::Value::Text(value) => {
                                SheetValue::String(value.clone())
                            }
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
                        style: get_cell_style_ods(w, s, &c.1, col, row),
                    })
                }
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
    if infer::doc::is_xls(data) || infer::doc::is_xlsx(data) {
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
            .map(|s| get_sheet_infos_ods(&workbook, s))
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
    if infer::doc::is_xls(data) || infer::doc::is_xlsx(data) {
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

                    Ok(get_sheet_infos_ods(&workbook, workbook.sheet(*index)))
                }
                SheetIdent::Name(name) => Ok(get_sheet_infos_ods(
                    &workbook,
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
