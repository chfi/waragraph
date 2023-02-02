# Waragraph - a variation graph visualizer


## Usage

Build with `cargo`. Only the latest version of Rust is supported.

```rust
cargo build --release
```

Run by providing a GFA file, and, optionally, a TSV layout file from [`odgi layout`](https://odgi.readthedocs.io/en/stable/rst/commands/odgi_layout.html).
A layout file can also be provided after the program has started, from the settings window.

```sh
./target/release/waragraph graph.gfa layout.tsv
```


### Global

Press `Escape` to open the settings window. If not provided on startup, a TSV layout file
can be loaded under "Graph & Layout" in the "General" tab.


### 1D

Scroll the path list by scrolling the mouse wheel with the cursor over the path names.
Zooming the view can be done by scrolling the wheel over the path visualizations.

Up and down arrow keys also scroll the list, and the left and right keys pan the view.
Spacebar resets the view.

### 2D

Pan and zoom the view by clicking and dragging with the mouse, and scrolling the mouse wheel.
Spacebar resets the view.


## Project structure

- `/lib` contains the core graph and related algorithms.
- `/app` contains the visualizer application.
