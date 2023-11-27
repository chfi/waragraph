
import init_wasm, * as wasm_bindgen from 'waragraph';
import * as Comlink from 'comlink';

import * as rxjs from 'rxjs';

import * as handler from './transfer_handlers';

// const handler = import('./transfer_handlers.js');

let _graph;
let _path_index;

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

    segmentAtOffset(bp) {
        return this.coord_sys.segment_at_pos(bp);
    }

    segmentOffset(segment) {
        return this.coord_sys.offset_at(segment);
    }

    segmentRange(segment) {
        return this.coord_sys.segment_range(segment);
    }

    viewMax() {
        return this.view.max;
    }

    subscribeTranslateDeltaNorm(observable) {
        let new_sub = observable.subscribe(delta => {
            let delta_bp = delta * this.view.len;
            this.translateView(delta_bp);
        });
    }

    subscribeCenterAt(observable) {
        let new_sub = observable.subscribe(bp_pos => {
            this.centerAt(bp_pos);
        });
    }

    subscribeZoomAround(observable) {
        let new_sub = observable.subscribe(({ scale, x }) => {
            this.zoomNorm(x, scale);
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

    set(new_range) {
        let start, end;

        if (new_range.start === undefined) {
            start = this.view.start;
        } else {
            start = new_range.start;
        }

        if (new_range.end === undefined) {
            end = this.view.end;
        } else {
            end = new_range.end;
        }

        this.view.set(start, end);
        console.log("[" + start + " - " + end + "]");
        this.push();

        let new_start = this.view.start;
        let new_end = this.view.end;

        // console.log("[" + new_start + " - " + new_end + "]");
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

    resizeTargetCanvas(width, height) {
        const valid = (v) => Number.isInteger(v) && v > 0;
        if (valid(width) && valid(height)) {
            this.path_viewer.resize_target_canvas(width, height);
        }
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

const path_coordinate_systems = new Map();

function getPathCoordinateSystem(path_name) {
    let cs = path_coordinate_systems.get(path_name);

    if (cs !== undefined) {
        return cs;
    }

    let path_index = _graph.path_index(path_name);
    let path_cs = wasm_bindgen.CoordSys.path_from_arrow_gfa(_graph, path_index);

    path_coordinate_systems.set(path_name, path_cs);

    return path_cs;
}

async function run(memory, gfa_path) {
    wasm = await init_wasm(undefined, memory);

    wasm_bindgen.set_panic_hook();

    console.log("fetching GFA");
    let gfa = fetch(gfa_path);
    
    console.log("parsing GFA");
    let timer0_ = Date.now();
    let graph = await wasm_bindgen.load_gfa_arrow(gfa);
    let timer1_ = Date.now();
    console.warn("parsing GFA took ", timer1_ - timer0_, " ms");

    let path_index = await graph.generate_path_index();
    _path_index = path_index;

    console.log("constructing coordinate system");
    let coord_sys = wasm_bindgen.CoordSys.global_from_arrow_gfa(graph);
    // let timer1_ = Date.now();
    // console.log(timer2_);
    // console.log("deriving depth data");
    // let data = wasm_bindgen.arrow_gfa_depth_data(graph, path_name);

    _state = { coord_sys };

    console.log(_state);

    _graph = graph;
    let view = wasm_bindgen.View1D.new_full(coord_sys.max());

    _global_cs_view = new CoordSysView(coord_sys, view);

    Comlink.expose({
        createPathViewer(offscreen_canvas,
                         path_name) {

            let coord_sys = _state.coord_sys;

            let data = wasm_bindgen.arrow_gfa_depth_data(graph, path_name);

            // let color_0 = 
            //     { r: 0.8, g: 0.3, b: 0.3, a: 1.0 };
            let color_0 = 
                { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

            let color_1 = wasm_bindgen.path_name_hash_color_obj(path_name);

            let opts = { bins: 1024, color_0, color_1 };

            let viewer = new PathViewerCtx(coord_sys, data, opts);

            viewer.connectCanvas(offscreen_canvas);

            return Comlink.proxy(viewer);
        },
        getPathNames() {
            let names = [];
            _graph.with_path_names((name) => {
                names.push(name);
            });
            return names;
        },
        globalCoordSysView() {
            return Comlink.proxy(_global_cs_view);
        },
        getGraph() {
            return _graph;
        },
        pathCoordSys(path_name) {
            return getPathCoordinateSystem(path_name);
        },
        pathsOnSegment(segment) {
            return path_index.paths_on_segment(segment);
        },
        pathIndex() {
            return path_index;
        },
        pathRangeToStepRange(path_name, range_start, range_end) {
            let start = typeof range_start == 'bigint'
                ? range_start : BigInt(range_start);
            let end = typeof range_end == 'bigint'
                ? range_end : BigInt(range_end);

            let path_cs = getPathCoordinateSystem(path_name);
            return path_cs.bp_to_step_range(start, end);
        }

    });

    postMessage("GRAPH_READY");
}


import('./transfer_handlers.js').then((handler) => {
    handler.setTransferHandlers(rxjs, Comlink);
    postMessage("WORKER_INIT");
});

onmessage = (event) => {
    onmessage = undefined;
    console.log(event);
    console.log("received message");
    console.log(typeof event.data);
    console.log(event.data);

    run(event.data[0], event.data[1]);
}
