# Spreet

Spreet is a spreadsheet decoder for typst (excel/opendocument spreadsheets).
In the normal mode the spreadsheet will be read and parsed into a dictonary of 2-dimensional array of strings:
Each workbook in the spreadsheet is mapped as an entry in the dictonary.
Each row of the workbook is represented as an array of strings, and all rows are summarised in a single array.
For full parsing for all information use the "full" option.

> [!WARNING]
> The ‘full’ option is currently in an unstable state. Fundamental changes (breaking changes) may occur.

(
  "sheetname": (range),
  index: (range),
)

!The library only supports normal tables. Charts are not supported.!


## Example

```typst
#import "@preview/spreet:0.2.0"

#let excel_data = spreet.file-decode("excel.xlsx")
#let opendocument_data = spreet.file-decode("opendocument.ods")

#let excel_data_from_bytes = spreet.decode(read("excel.xlsx", encoding: none))
#let opendocument_data_from_bytes = spreet.decode(read("opendocument.ods", encoding: none))

#let excel_data_with_index = spreet.file-decode("excel.xlsx", index: 0)
#let opendocument_data_with_index_name = spreet.file-decode("opendocument.ods", index: "name")

/**
excel_data or opendocument_data contains a dict of all worksheets (of the selected worksheet)
(
  Worksheet1: (
    (Row1_Column1, Row1_Column2),
    (Row2_Column1, Row2_Column2),
  ),
  Worksheet2: (
    (Row1_Column1, Row1_Column2),
    (Row2_Column1, Row2_Column2),
  )
)
**/

// for full decoding with all information use the "full" parameter

#let excel_data = spreet.file-decode("excel.xlsx", full: true)
#let opendocument_data = spreet.file-decode("opendocument.ods", full: true)

#let excel_data_from_bytes = spreet.decode(read("excel.xlsx", encoding: none), full: true)
#let opendocument_data_from_bytes = spreet.decode(read("opendocument.ods", encoding: none), full: true)

/**
(
  (
    Name: Worksheet1,
    ....
    Data: (
      ....
    )
  ),
  (
    Name: Worksheet2,
    ....
    Data: (
      ....
    )
  )
)
*/
```