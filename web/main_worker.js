
import init_wasm, * as wasm_bindgen from './pkg/web.js';
import * as Comlink from './comlink.mjs';


import importUMD from './importUMD.js';

const rxjs = await importUMD('./rxjs.umd.min.js');

const handler = await import('./transfer_handlers.js');

handler.setTransferHandlers(rxjs, Comlink);

console.log("rxjs");
console.log(rxjs);

console.log(wasm_bindgen);
console.log(typeof wasm_bindgen);

let _graph;

let _state;

let wasm;

let _global_cs_view = null;

class CoordSysView {
    constructor(coord_sys, view) {
        this.coord_sys = coord_sys;
        this.view = view;

        let view_range = { start: view.start, end: view.end };

        this.view_range_subject = new rxjs.BehaviorSubject(view_range);
    }

    viewMax() {
        return this.view.max;
    }

    subscribeTranslateDeltaNorm(observable) {
        let new_sub = observable.subscribe(delta => {
            console.log("in sub");
            let delta_bp = delta * this.view.len;
            this.translateView(delta_bp);
        });
    }

    subscribeCenterAt(observable) {
        let new_sub = observable.subscribe(bp_pos => {
            this.centerAt(bp_pos);
        });
    }

    subscribeZoomCentered(observable) {
        let new_sub = observable.subscribe(scale => {
            this.zoomViewCentered(scale);
        });
    }

    viewSubject() {
        return this.view_range_subject;
    }

    push() {
        let start = this.view.start;
        let end = this.view.end;
        this.view_range_subject.next({start, end});
    }

    max() {
        return this.view.max;
    }

    get() {
        let start = this.view.start;
        let end = this.view.end;
        let max = this.view.max;
        let len = this.view.len;
        return { start, end, max, len };
    }

    centerAt(bp) {
        // console.log("centering view around " + bp);
        let len = this.view.len;
        let start = bp - (len / 2);
        let end = bp + (len / 2);
        // console.log("left: " + left + ", right: " + right);
        this.view.set(start, end);
        this.push();
    }

    zoomNorm(norm_x, scale) {
        this.view.zoom_with_focus(norm_x, scale);
        this.push();
    }

    zoomViewCentered(scale) {
        this.view.zoom_with_focus(0.5, scale);
        this.push();
    }

    translateView(delta_bp) {
        this.view.translate(delta_bp);
        console.log("translating view");
        this.push();
    }

}




class PathViewerCtx {
    constructor(coord_sys, data, { bins, color_0, color_1}) {
        this.path_viewer = wasm_bindgen.PathViewer.new(coord_sys, data, bins, color_0, color_1);
        this.coord_sys = coord_sys;
    }

    connectCanvas(offscreen_canvas) {
        console.log(offscreen_canvas);
        this.path_viewer.set_target_canvas(offscreen_canvas);
    }

    setCanvasWidth(width) {
        this.path_viewer.set_offscreen_canvas_width(width);
    }

    forceRedraw(resample) {
        if (resample) {
            this.path_viewer.sample_range(this.view.start, this.view.end);
        }
        this.path_viewer.draw_to_canvas();
    }


    coordSys() {
        return this.path_viewer.coord_sys;
    }

    setView(start, end) {
        this.view = { start, end };
    }

    sample() {
        let l = this.view.start;
        let r = this.view.end;
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
        let view = wasm_bindgen.View1D.new_full(coord_sys.max());

    _global_cs_view = new CoordSysView(coord_sys, view);
    // console.log("global cs view: ");
    // console.log(_global_cs_view);

    Comlink.expose({
        createPathViewer(offscreen_canvas,
                         path_name) {
            console.log("in createPathViewer");
            console.log(path_name);
            // console.log("in createPathViewer with " + path_name);
            // let path_name = "gi|528476637:29857558-29915771";
            let coord_sys = _state.coord_sys;
            console.log("getting coord_sys: " + coord_sys);

            console.log("deriving depth data");
            let data = wasm_bindgen.arrow_gfa_depth_data(graph, path_name);

            // let color_0 = 
            //     { r: 0.8, g: 0.3, b: 0.3, a: 1.0 };
            let color_0 = 
                { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

            let color_1 = wasm_bindgen.path_name_hash_color_obj(path_name);
            console.log("color_1: " + color_1);
            console.log(color_1);

            let opts = { bins: 1024, color_0, color_1 };

            let viewer = new PathViewerCtx(coord_sys, data, opts);

            viewer.connectCanvas(offscreen_canvas);

            return Comlink.proxy(viewer);
        },

        // connectCanvas(offscreen_canvas) {
        //     _state.path_viewer.connectCanvas(offscreen_canvas);
        // },

        globalCoordSys() {
            return Comlink.proxy(_global_cs_view);
        },

        getPathNames() {
            let names = [];
            _graph.with_path_names((name) => {
                names.push(name);
            });
            return names;
        },

        // sampleRange(left, right) {
        //     _state.path_viewer.setView(left, right);
        //     _state.path_viewer.sample();
        // },

        getGraph() {
            return Comlink.proxy(_graph);
        }
    });

    postMessage("graph-ready");
}

run();
