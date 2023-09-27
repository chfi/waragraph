

### building

you need [`wasm-pack`](https://rustwasm.github.io/wasm-pack/installer/) and `npm`

make sure you have the rust source code installed:

`rustup target add wasm32-unknown-unknown`
`rustup toolchain install nightly-2023-09-26`
`rustup component add rust-src --toolchain nightly-2023-09-26`

build with `wasm-pack build --target web`, serve this folder using a
server that sets CORS headers, e.g. using the provided python script `python server.py`
