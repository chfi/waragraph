
import init_wasm, * as wasm_bindgen from './pkg/web.js';
import * as Comlink from './comlink.mjs';


import importUMD from './importUMD.js';

const rxjs = await importUMD('./rxjs.umd.min.js');

console.log("rxjs");
console.log(rxjs);

console.log(wasm_bindgen);
console.log(typeof wasm_bindgen);

let _graph;

let _state;

let wasm;


class ViewProxy {
    constructor(view) {
        this.view = view;
    }

    max() {
        return this.view.max;
    }

    get() {
        let left = this.view.start;
        let right = this.view.end;
        let max = this.view.max;
        let len = this.view.len;
        return { left, right, max, len };
    }

    centerAt(bp) {
        // console.log("centering view around " + bp);
        let len = this.view.len;
        let left = bp - (len / 2);
        let right = bp + (len / 2);
        // console.log("left: " + left + ", right: " + right);
        this.view.set(left, right);
    }

    zoomNorm(norm_x, scale) {
        this.view.zoom_with_focus(norm_x, scale);

    }

    zoomViewCentered(scale) {
        this.view.zoom_with_focus(0.5, scale);
    }

    translateView(delta_bp) {
        this.view.translate(delta_bp);
    }

}


class PathViewerCtx {
    constructor(coord_sys, data, { bins, color_0, color_1}) {
        // TODO set view based on coord_sys, or take an optional argument

        let max = coord_sys.max();
        console.log("coord_sys max: " + max);
        let view = wasm_bindgen.View1D.new_full(coord_sys.max());
        console.log("okay");
        // let view = { left: 0, right: coord_sys.max() };
        this.path_viewer = wasm_bindgen.PathViewer.new(coord_sys, data, bins, color_0, color_1);
        this.view = view;
        this.coord_sys = coord_sys;
    }

    connectCanvas(offscreen_canvas) {
        console.log(offscreen_canvas);
        this.path_viewer.set_target_canvas(offscreen_canvas);
    }

    forceRedraw(resample) {
        if (resample) {
            this.path_viewer.sample_range(this.view.start, this.view.end);
        }
        this.path_viewer.draw_to_canvas();
    }

    maxView() {
        return this.view.max;
    }

    coordSys() {
        return this.path_viewer.coord_sys;
    }

    setView(left, right) {
        this.view.set(left, right);
    }

    getView() {
        let left = this.view.start;
        let right = this.view.end;
        let max = this.view.max;
        return { left, right, max };
    }

    centerViewAt(bp) {
        console.log("centering view around " + bp);
        let len = this.view.len;
        let left = bp - (len / 2);
        let right = bp + (len / 2);
        console.log("left: " + left + ", right: " + right);
        this.view.set(left, right);
    }

    zoomViewNorm(norm_x, scale) {
        this.view.zoom_with_focus(norm_x, scale);
    }

    zoomViewCentered(scale) {
        this.view.zoom_with_focus(0.5, scale);
    }

    translateView(delta_bp) {
        this.view.translate(delta_bp);
        /*
        let left = this.view.start();
        let right = this.view.end();
        // let { left, right } = this.view;
        let view_size = right - left + 1;
        
        let new_left = Math.round(left + delta_bp);

        if (new_left < 0) {
            new_left = 0;
        }

        if ((new_left + view_size) > this.path_viewer.coord_sys.max()) {
            new_left = this.path_viewer.coord_sys.max() - view_size - 1;
        }

        let new_right = new_left + view_size - 1;

        let new_size = new_right - new_left + 1;

        this.view = { left: new_left, right: new_right };

        console.log("old size: " + view_size + ", new size: " + new_size);
        console.log("old left: " + left + ", new left: " + new_left);
        */
    }

    sample() {
        let l = this.view.start;
        let r = this.view.end;
        let size = this.view.len;
        console.log("sampling: (" + l + ", " + r + ") [" + size + "]");
        this.path_viewer.sample_range(this.view.start, this.view.end);
    }
    
}


async function run() {
    wasm = await init_wasm();

    console.log(wasm_bindgen);

    wasm_bindgen.set_panic_hook();

    const gfa_path = '../data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa';
    // const gfa_path = './cerevisiae.pan.fa.gz.d1a145e.417fcdf.7493449.smooth.final.gfa';

    console.log("fetching GFA");

    let gfa = fetch(gfa_path);
    
    console.log("parsing GFA");
    // let ctx = wasm_bindgen.initialize_with_data_fetch(gfa, tsv
    let graph = await wasm_bindgen.load_gfa_arrow(gfa);

    let path_name = graph.path_name(0);
    // let path_name = "gi|528476637:29857558-29915771";
    console.log("constructing coordinate system");
    let coord_sys = wasm_bindgen.CoordSys.global_from_arrow_gfa(graph);
    console.log("deriving depth data");
    let data = wasm_bindgen.arrow_gfa_depth_data(graph, path_name);

    console.log("initializing path viewer");

    let color_0 = 
        { r: 0.8, g: 0.3, b: 0.3, a: 1.0 };
    let color_1 =
        { r: 0.2, g: 0.2, b: 0.2, a: 1.0 };

    let opts = { bins: 1024, color_0, color_1 };

    let path_viewer = new PathViewerCtx(coord_sys, data, opts);

    coord_sys = path_viewer.path_viewer.coord_sys;

    console.log("_state coord_sys: " + coord_sys);

    _state = { path_name, path_viewer, coord_sys };

    console.log(_state);

    _graph = graph;
    console.log("worker node count: " + _graph.segment_count());


    Comlink.expose({
        createPathViewer(offscreen_canvas, path_name) {
            console.log("in createPathViewer with " + path_name);
            // let path_name = "gi|528476637:29857558-29915771";
            let coord_sys = _state.coord_sys;
            console.log("getting coord_sys: " + coord_sys);

            console.log("deriving depth data");
            let data = wasm_bindgen.arrow_gfa_depth_data(graph, path_name);

            let color_0 = 
                { r: 0.8, g: 0.3, b: 0.3, a: 1.0 };
            let color_1 =
                { r: 0.2, g: 0.2, b: 0.2, a: 1.0 };

            let opts = { bins: 1024, color_0, color_1 };

            let viewer = new PathViewerCtx(coord_sys, data, opts);

            viewer.connectCanvas(offscreen_canvas);

            return Comlink.proxy(viewer);
        },

        connectCanvas(offscreen_canvas) {
            _state.path_viewer.connectCanvas(offscreen_canvas);
        },

        sampleRange(left, right) {
            _state.path_viewer.setView(left, right);
            _state.path_viewer.sample();
        },

        get_graph() {
            return Comlink.proxy(_graph);
        }
    });

    postMessage("graph-ready");
}

run();
