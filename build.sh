if [ ! -d "binaryen-version_129" ]; then
    curl -L https://github.com/WebAssembly/binaryen/releases/download/version_129/binaryen-version_129-x86_64-linux.tar.gz | tar zx
fi
cd wasm-minimal-protocol/crates/wasi-stub
cargo build --release
cd ../../..
rustup target add wasm32-wasip1
cp README.md typst-package/
cp LICENSE typst-package/
cargo build --release --target wasm32-wasip1
wasm-minimal-protocol/target/release/wasi-stub -r 0 ./target/wasm32-wasip1/release/spreet.wasm -o typst-package/spreet.wasm
binaryen-version_122/bin/wasm-opt typst-package/spreet.wasm -O3 --enable-bulk-memory -o typst-package/spreet.wasm 