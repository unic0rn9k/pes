serve package: (build package)
    miniserve --index index.html ./target/{{package}}/

build package:
    cargo build -j8 --release --package {{package}} --target wasm32-unknown-unknown --features web
    wasm-bindgen --target web --no-typescript --out-dir ./ ./target/wasm32-unknown-unknown/release/{{package}}.wasm

clean package:
    rm -rf ./target/{{package}}/
