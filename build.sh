rustup target add wasm32-wasi
cp README.md typst-package/
cp LICENSE typst-package/
cargo build --release --target wasm32-wasi
wasi-stub -r 0 ./target/wasm32-wasi/release/spreet.wasm -o typst-package/spreet.wasm
wasm-opt typst-package/spreet.wasm -O3 --enable-bulk-memory -o typst-package/spreet.wasm