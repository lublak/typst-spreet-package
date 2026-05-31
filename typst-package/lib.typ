#let spreet = plugin("spreet.wasm")

#let decode(
  data,
  options: (
    sheets: (),
    full: false,
  ),
) = {
  if options.full {
    if options.at("sheets", default: ()).len() == 0 {
      cbor(spreet.decode_full(data))
    } else {
      cbor(spreet.decode_full_with_indexes(data, options.sheets))
    }
  } else {
    if options.at("sheets", default: ()).len() == 0 {
      cbor(spreet.decode(data))
    } else {
      cbor(spreet.decode_with_indexes(data, options.sheets))
    }
  }
}
