#let spreet = plugin("spreet.wasm")

#let decode(
  data,
  options: (
    sheets: (),
    full: false,
  ),
) = {
  if options.full {
    if options.sheets.len() == 0 {
      cbor.decode(spreet.decode_full(data))
    } else {
      cbor.decode(spreet.decode_full_with_indexes(data, options.sheets))
    }
  } else {
    if options.sheets.len() == 0 {
      cbor.decode(spreet.decode(data))
    } else {
      cbor.decode(spreet.decode_with_indexes(data, options.sheets))
    }
  }
}

#let file-decode(
  path,
  options: (
    sheets: (),
    full: false,
  ),
) = {
  decode(read(path, encoding: none), options: options)
}
