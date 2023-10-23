

### building

NB: until a new release of wasm-bindgen is out, wasm-bindgen-cli must be installed from git:
`cargo install wasm-bindgen-cli --git https://github.com/rustwasm/wasm-bindgen.git --rev 2e9ff5dfa3f11415f0efe9b946ee2734500e9ee3`

you need [`wasm-pack`](https://rustwasm.github.io/wasm-pack/installer/) and `npm`

make sure you have the rust source code installed:

`rustup target add wasm32-unknown-unknown`
`rustup toolchain install nightly-2023-09-26`
`rustup component add rust-src --toolchain nightly-2023-09-26`

build with `wasm-pack build --target web`, serve this folder using a
server that sets CORS headers, e.g. using the provided python script `python server.py`
