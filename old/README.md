# waragraph

a variation graph viewer of sorts

### Requirements

waragraph uses the Vulkan API, make sure you have the SDK installed: https://vulkan.lunarg.com/sdk/home

If you use mac, you'll need to install [MoltenVK](https://github.com/KhronosGroup/MoltenVK)

### Building and running

Make sure you have the latest version of
[`Rust`](https://www.rust-lang.org/tools/install) installed, and build
using `cargo`:

```
git clone https://github.com/chfi/waragraph.git
cd waragraph
cargo build --release
./target/release/waragraph input.gfa
```

Use the arrow keys and page up/down to navigate the view
