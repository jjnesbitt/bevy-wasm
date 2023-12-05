## Building the client

```
cargo build -p client --release --target wasm32-unknown-unknown
wasm-bindgen --out-dir ./client/static/ --target web --no-typescript ./target/wasm32-unknown-unknown/release/client.wasm
```

### Serving the static files
```
python -m http.server --directory ./client/static/
```

## Running the server
```
cargo run -p server
```