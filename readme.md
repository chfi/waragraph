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

Annotations can be loaded at startup using the `--bed` or `--gff` (in combination with `--gff-attr`) command line arguments. 

When a GFF file is loaded, the attribute key from `--gff-attr` is used as the label.

4 column BED files are supported, with the 4th column being used as
the label. If the 4th column ends with a space followed by a hex-coded
color, e.g. “SomeGene #32ABCD”, that will be the annotation’s
highlight color.


```sh
./target/release/waragraph graph.gfa layout.tsv --bed some.bed
```

### Global

Press `Escape` to open and close the settings window. If not provided on startup, a TSV layout file
can be loaded under "Graph & Layout" in the "General" tab.


### 1D

Scroll the path list by scrolling the mouse wheel with the cursor over the path names.
Zooming the view can be done by scrolling the wheel over the path visualizations.

Right click on a node to pan the 2D view to that node. This does not zoom the 2D view.

Up and down arrow keys also scroll the list, and the left and right keys pan the view.
Press `Space` to reset the view.


### 2D

Pan and zoom the view by clicking and dragging with the mouse, and scrolling the mouse wheel.

Right click on a node to pan the 1D view to that node. This does not
zoom the 1D view, so if the 1D view is fully zoomed out, nothing will
happen.

If annotations are loaded, left clicking an annotation in the sidebar
list will pan the view to it, and right clicking it will toggle it so
that it’s always highlighted.

Press `Space` to reset the view.



## Project structure

- `/lib` contains the core graph and related algorithms.
- `/app` contains the visualizer application.
