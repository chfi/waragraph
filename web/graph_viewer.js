import * as rxjs from 'rxjs';
import { mat3, vec3 } from 'gl-matrix';

import init_module, * as wasm_bindgen from './pkg/web.js';

import { placeTooltipAtPoint } from './tooltip.js';

/*
  wgpu/webgl seems to be troublesome with workers, and it's not clear
  what the current state of things is, exactly.
  The GraphViewer class initializes its own wasm module (same code as the rest of the web app),
  sharing the memory initialized in `main_worker.js`, but lives on the main thread,
  and takes care of rendering the 2D graph view.
*/

let wasm;
let _raving_ctx;

class GraphViewer {
    constructor(viewer, seg_pos) {
        // maybe just take the minimum raw data needed here
        this.graph_viewer = viewer;
        this.segment_positions = seg_pos;

        this.next_view = this.graph_viewer.get_view();

        this.overlayCallbacks = {};
        this.mousePos = null;
    }

    needRedraw() {
        return !this.next_view.equals(this.graph_viewer.get_view());
    }

    lookup(x, y) {
        try {
            let val = this.graph_viewer.gbuffer_lookup(_raving_ctx, x, y);
            console.log(val);
            return val;
        } catch (e) {
            return null;
        }
    }

    fitViewToGraph() {
        let graph_bounds = this.segment_positions.bounds_as_view_obj();
        // let view_obj = this.next_view.as_obj();

        let canvas = document.getElementById("graph-viewer-2d");
        let view_width, view_height;

        // if (graph_bounds.width > graph_bounds.height) {
            view_width = graph_bounds.width;
            view_height = graph_bounds.width * (canvas.height / canvas.width);
        // } else {
            // let c_aspect = canvas.width / canvas.height;

            // view_width = graph_bounds.height * c_aspect;
            // view_height = graph_bounds.height;
        // }

        this.next_view.set_center(graph_bounds.x, graph_bounds.y);
        this.next_view.set_size(view_width, view_height);
    }

    resize() {
        // let el = document.getElementById('graph-viewer-2d-overlay');
        let el = document.getElementById('graph-viewer-2d-overlay');
        // let container = el.parentNode;

        let width = el.parentNode.clientWidth;
        let height = el.parentNode.clientHeight;

        el.width = width;
        el.height = height;

        this.graph_viewer.resize(_raving_ctx, Math.round(width), Math.round(height));
        this.fitViewToGraph();
    }

    draw() {
        this.graph_viewer.set_view(this.next_view);
        this.graph_viewer.draw_to_surface(_raving_ctx);

        this.drawOverlays();
    }

    drawOverlays() {
        let overlay = document
            .getElementById('graph-viewer-2d-overlay');
        let ctx = overlay.getContext('2d');

        ctx.clearRect(0, 0, overlay.width, overlay.height);

        console.log(this.overlayCallbacks);
        for (const key in this.overlayCallbacks) {
            const callback = this.overlayCallbacks[key];
            callback(overlay, this.next_view, this.mousePos);
        }
    }

    registerOverlayCallback(cb_key, callback) {
        this.overlayCallbacks[cb_key] = callback;
        this.drawOverlays();
    }

    removeOverlayCallback(cb_key) {
        delete this.overlayCallbacks[cb_key];
        this.drawOverlays();
    }

    translate(x, y) {
        this.next_view.translate_size_rel(x, y);
    }

    zoom(tx, ty, s) {
        this.next_view.zoom_with_focus(tx, ty, s);
    }

    get_view_matrix() {
        return this.graph_viewer.get_view_matrix();
    }

    get_segment_pos(seg) {
        return this.segment_positions.segment_pos(seg);
    }
}

export { GraphViewer };

let _wasm;

// initializing raving/wgpu works when done here, but not when
// using the wasm memory shared from the worker
export async function initGraphViewer(wasm_mem, graph, layout_url) {
    console.log(">>>>>>>>>> in testRavingCtx");
    if (_wasm === undefined) {
        console.log("initializing with memory: ");
        console.log(wasm_mem);
        console.log(wasm_mem.buffer.byteLength);
        _wasm = await init_module(undefined, wasm_mem);
        wasm_bindgen.set_panic_hook();
    }

    if (_raving_ctx === undefined) {
        console.log("initializing raving ctx");

        let canvas = document.getElementById('graph-viewer-2d');

        _raving_ctx = await wasm_bindgen.RavingCtx.initialize_(canvas);
    }

    console.log("creating segment positions");

    let layout_tsv = await fetch(layout_url).then(l => l.text());
    let seg_pos = wasm_bindgen.SegmentPositions.from_tsv(layout_tsv);

    console.log("created segment positions");

    let _graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph.__wbg_ptr);
    let seg_count = _graph.segment_count();
    console.log("segment count: " + seg_count);

    let canvas = document.getElementById("graph-viewer-2d");

    let container = document.getElementById("graph-viewer-container");
    let dims = { width: container.clientWidth, height: container.clientHeight };

    let viewer = wasm_bindgen.GraphViewer.new_dummy_data(_raving_ctx,
          _graph,
          seg_pos,
          canvas);

    canvas.width = dims.width;
    canvas.height = dims.height;

    let overlay = document.getElementById('graph-viewer-2d-overlay');

    overlay.width = dims.width;
    overlay.height = dims.height;

    viewer.resize(_raving_ctx, dims.width, dims.height);

    viewer.draw_to_surface(_raving_ctx);

    let graph_viewer = new GraphViewer(viewer, seg_pos);

    const draw_loop = () => {
        if (graph_viewer.needRedraw()) {
            graph_viewer.draw();
        }

        window.requestAnimationFrame(draw_loop);
    };


    draw_loop();
    
    ////

    const mouseDown$ = rxjs.fromEvent(overlay, 'mousedown');
    const mouseUp$ = rxjs.fromEvent(overlay, 'mouseup');
    const mouseOut$ = rxjs.fromEvent(overlay, 'mouseout');
    const mouseMove$ = rxjs.fromEvent(overlay, 'mousemove');


    mouseMove$.subscribe((event) => {
        graph_viewer.mousePos = { x: event.offsetX, y: event.offsetY };
    });

    mouseOut$.subscribe((event) => {
        graph_viewer.mousePos = null;
    });

    const drag$ = mouseDown$.pipe(
        rxjs.switchMap((event) => {
            return mouseMove$.pipe(
                rxjs.pairwise(),
                rxjs.map(([prev, current]) => [current.offsetX - prev.offsetX,
                                               current.offsetY - prev.offsetY]),
                rxjs.takeUntil(
                    rxjs.race(mouseUp$, mouseOut$)
                )
            )
        })
    );

    drag$.subscribe(([dx, dy]) => {
        let x = dx / overlay.width;
        let y = dy / overlay.height;
        graph_viewer.translate(-x, y);
    });

    const wheel$ = rxjs.fromEvent(overlay, 'wheel').pipe(
        rxjs.tap(event => event.preventDefault())
    );

    wheel$.subscribe((event) => {
        let x = event.offsetX;
        let y = overlay.height - event.offsetY;

        let nx = x / overlay.width;
        let ny = y / overlay.height;

        let scale = event.deltaY > 0.0 ? 1.05 : 0.95;

        graph_viewer.zoom(nx, ny, scale);
    });

    graph_viewer.fitViewToGraph();

    /*
    {

        let path_cs = wasm_bindgen.CoordSys.path_from_arrow_gfa(_graph, path_i);

        let path_offset = 28510128n;
        let path_len = 33480000n - path_offset
        let path_name = "grch38#chr6:28510128-33480000";
        let path_i = _graph.path_index(path_name);

        let gene = {
            start: 32459821n,
            end: 32473500n,
            label: "HLA-DRB9"
        };

        let bp_range = { start: gene.start - path_offset,
                         end: gene.end - path_offset };

        let range = path_cs.bp_to_step_range(bp_range.start, bp_range.end);
        let path_steps = _graph.path_steps(path_name);

        const draw_path_slice = () => {
            let { start, end } = range;
            let view = graph_viewer.graph_viewer.get_view();

            let path_slice = path_steps.slice(start, end);
            let path2d = seg_pos.path_to_canvas_space(view, overlay.width, overlay.height, path_slice);

            let ov_ctx = overlay.getContext('2d');
            ov_ctx.save();
            ov_ctx.globalAlpha = 0.8;
            ov_ctx.globalCompositeOperation = "copy";
            ov_ctx.lineWidth = 15;
            ov_ctx.strokeStyle = 'black';
            ov_ctx.stroke(path2d);
            ov_ctx.lineWidth = 10;
            ov_ctx.strokeStyle = 'red';
            ov_ctx.stroke(path2d);
            ov_ctx.restore();
        };

        graph_viewer.draw_path_slice = draw_path_slice;
    }
    */


    /*
    let path_name = "HG00438#1#h1tg000040l:22870040-27725000";
    let path = _graph.path_steps(path_name);

    const draw_path = () => {
        let view = graph_viewer.graph_viewer.get_view();
        let path2d = seg_pos.path_to_canvas_space(view, 800, 600, path);

        let ov_ctx = overlay.getContext('2d');
        ov_ctx.stroke(path2d);
    };

    graph_viewer.draw_path = draw_path;
    */


    /*
    // strokes a (canvas) path along a (graph) path
    let path_name = "gi|157734152:29655295-29712160";
    let path = _graph.path_steps(path_name);

    let view = graph_viewer.viewer.get_view();

    let path2d = seg_pos.path_to_canvas_space(view, 800, 600, path);

    let ov_ctx = overlay.getContext('2d');
    ov_ctx.stroke(path2d);
    */



    ////

    return graph_viewer;
}

let pathHighlightTolerance = 5;

export function getPathTolerance() {
    return pathHighlightTolerance;
}

export function setPathTolerance(tol) {
    pathHighlightTolerance = tol;
}

// input ranges should be in path space
export function preparePathHighlightOverlay(seg_pos, path_steps, path_cs_raw, entries) {
    const path_cs = wasm_bindgen.CoordSys.__wrap(path_cs_raw.__wbg_ptr);

    const processed = [];

    for (const entry of entries) {
        const { start, end, label } = entry;
        const step_range = path_cs.bp_to_step_range(BigInt(start), BigInt(end));
        const path_slice = path_steps.slice(step_range.start, step_range.end);

        processed.push({ path_slice, color: entry.color, start, end, label });
        console.log("steps in ", entry.label, ": ", path_slice.length);
    }

    // console.log(processed);

    return (canvas, view, mouse_pos) => {

        /*
        {
            let ctx = canvas.getContext('2d');
            ctx.save();

            // const path = new Path2D();
            // path.ellipse(150, 75, 40, 60, Math.PI * 0.25, 0, 2 * Math.PI);
            // ctx.strokeRect(0, 0, 300, 300);
            // ctx.stroke(path);
            ctx.globalAlpha = 1.0;
            ctx.fillStyle = 'black';
            ctx.fillRect(0, 0, 300, 300);

            ctx.restore();
        }
        */

        let view_matrix = view.to_js_mat3(canvas.width, canvas.height);
        // console.log(view_matrix);

            let ctx = canvas.getContext('2d');
            ctx.save();

        for (const entry of processed) {

            try {
                let canv_path = seg_pos.sample_canvas_space_path(
                    view,
                    canvas.width,
                    canvas.height,
                    entry.path_slice,
                    pathHighlightTolerance,
                )

                // TODO handle zero length path cases
                let len = canv_path.length;
                console.warn("canvas path length: ", len);

                ctx.beginPath();

                let start = canv_path.get_point(0);
                ctx.moveTo(start.x, start.y);

                canv_path.with_points((x, y) => {
                    ctx.lineTo(x, y);
                });

                ctx.globalAlpha = 0.8;
                // ctx.globalCompositeOperation = "copy";
                ctx.lineWidth = 15;
                ctx.strokeStyle = entry.color;
                ctx.stroke();

                /*
                if (ctx.isPointinStroke(mouse_pos.x, mouse_pos.xy)) {
                    console.log(entry.label);
                    // tooltip.innerHTML = `Segment ${segment}`;
                    // tooltip.style.display = 'block';
                    // placeTooltipAtPoint(x, y);
                }
                */
                ctx.closePath();

                // ctx.strokeStyle = 'black';
                // ctx.fillStyle = 'black';

                let ends = canv_path.get_endpoints();

                if (ends !== null) {
                    console.warn(ends);

                    let x = ends.start.x + (ends.end.x - ends.start.x) * 0.5;
                    let y = ends.start.y + (ends.end.y - ends.start.y) * 0.5;
                    ctx.fillText(entry.label, x, y);
                }

            } catch (e) {
                console.error("oh no: ", e);
                //
            }


        }

            ctx.restore();
    };
}




function resize_view_dimensions(v_dims, c_old, c_new) {
    let [v_w, v_h] = v_d;
    let [c_old_w, c_old_h] = c_old;
    let [c_new_w, c_new_h] = c_new;

    let S_w = c_new_w / c_old_w;
    let S_h = c_new_h / c_old_h;
    let S = Math.min(S_w, S_h);

    let v_new_w = v_w * S;
    let v_new_h = v_h * S;

    return [v_new_h, v_new_h];
}



export async function initializeGraphViewer(wasm_mem, graph_raw, layout_url) {
    if (_wasm === undefined) {
        _wasm = await init_module(undefined, wasm_mem);
        wasm_bindgen.set_panic_hook();
    }

    // create canvases

    let gpu_canvas = document.createElement('canvas');
    let overlay_canvas = document.createElement('canvas');

    gpu_canvas.id = 'graph-viewer-2d';
    overlay_canvas.id = 'graph-viewer-2d-overlay';

    gpu_canvas.style.setProperty('z-index', 0);
    overlay_canvas.style.setProperty('z-index', 1);
    // gpu_canvas.style.setProperty('z-index', '0');
    // overlay_canvas.style.setProperty('z-index', '1');

    let container = document.getElementById('graph-viewer-container');

    container.append(gpu_canvas);
    container.append(overlay_canvas);

    let width = container.clientWidth;
    let height = container.clientHeight;

    gpu_canvas.width = container.clientWidth;
    gpu_canvas.height = container.clientHeight;
    overlay_canvas.width = container.clientWidth;
    overlay_canvas.height = container.clientHeight;

    if (_raving_ctx === undefined) {
        // let canvas = document.getElementById('graph-viewer-2d');
        _raving_ctx = await wasm_bindgen.RavingCtx.initialize_(gpu_canvas);
    }

    let layout_tsv = await fetch(layout_url).then(l => l.text());
    let seg_pos = wasm_bindgen.SegmentPositions.from_tsv(layout_tsv);

    let graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph_raw.__wbg_ptr);

    let viewer = wasm_bindgen.GraphViewer.new_dummy_data(
        _raving_ctx,
        graph,
        seg_pos,
        gpu_canvas
    );

    viewer.resize(_raving_ctx, width, height);
    viewer.draw_to_surface(_raving_ctx);

    let graph_viewer = new GraphViewer(viewer, seg_pos);
        
    const draw_loop = () => {
        if (graph_viewer.needRedraw()) {
            graph_viewer.draw();
        }

        window.requestAnimationFrame(draw_loop);
    };

    draw_loop();

    const mouseDown$ = rxjs.fromEvent(overlay_canvas, 'mousedown');
    const mouseUp$ = rxjs.fromEvent(overlay_canvas, 'mouseup');
    const mouseOut$ = rxjs.fromEvent(overlay_canvas, 'mouseout');
    const mouseMove$ = rxjs.fromEvent(overlay_canvas, 'mousemove');


    const hoveredSegment$ = mouseMove$.pipe(
        rxjs.map((ev) => ({ x: ev.offsetX, y: ev.offsetY })),
        rxjs.distinct(),
        rxjs.throttleTime(40),
        rxjs.map(({x, y}) => graph_viewer.lookup(x, y)),
    );

    hoveredSegment$.subscribe((segment) => {
        if (segment !== null) {
            // hovered segment
        }
    })

    mouseMove$.subscribe((event) => {
        graph_viewer.mousePos = { x: event.offsetX, y: event.offsetY };
    });

    mouseOut$.subscribe((event) => {
        graph_viewer.mousePos = null;
    });

    const drag$ = mouseDown$.pipe(
        rxjs.switchMap((event) => {
            return mouseMove$.pipe(
                rxjs.pairwise(),
                rxjs.map(([prev, current]) => [current.offsetX - prev.offsetX,
                                               current.offsetY - prev.offsetY]),
                rxjs.takeUntil(
                    rxjs.race(mouseUp$, mouseOut$)
                )
            )
        })
    );

    drag$.subscribe(([dx, dy]) => {
        let x = dx / overlay_canvas.width;
        let y = dy / overlay_canvas.height;
        graph_viewer.translate(-x, y);
    });

    const wheel$ = rxjs.fromEvent(overlay_canvas, 'wheel').pipe(
        rxjs.tap(event => event.preventDefault())
    );

    wheel$.subscribe((event) => {
        let x = event.offsetX;
        let y = overlay_canvas.height - event.offsetY;

        let nx = x / overlay_canvas.width;
        let ny = y / overlay_canvas.height;

        let scale = event.deltaY > 0.0 ? 1.05 : 0.95;

        graph_viewer.zoom(nx, ny, scale);
    });

    graph_viewer.fitViewToGraph();


    return graph_viewer;
}
