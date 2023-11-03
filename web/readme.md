### building

you need [`wasm-pack`](https://rustwasm.github.io/wasm-pack/installer/) and `npm`

make sure you have the rust source code installed:

`rustup target add wasm32-unknown-unknown`
`rustup toolchain install nightly-2023-11-01`
`rustup component add rust-src --toolchain nightly-2023-11-01`

build with `wasm-pack build --target web`, run `npm install` to get the JS dependencies,
then start the dev server with parcel: `npx parcel index.html`
