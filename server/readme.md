
### Usage

First build the frontend, follow the instructions in `../web/readme.md`

Use `npx parcel build index.html`

Build the server from this folder using `cargo build --release`

Then start the server from this folder by providing GFA and layout TSV, or by using the provided `run.sh`

```
cargo run --release -- <GFA path> <TSV path>
./run.sh <GFA path> <TSV path>
```
