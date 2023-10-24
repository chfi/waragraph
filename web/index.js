import * as Comlink from './comlink.mjs';

const { Observable } = rxjs;

import { initializePathViewer, addOverviewEventHandlers, addPathViewerLogic } from './path_viewer_ui.js';

import init_module, * as wasm_bindgen from './pkg/web.js';

import { OverviewMap } from './overview.js';

import { GraphViewer, initGraphViewer } from './graph_viewer.js';

import * as CanvasTracks from './canvas_tracks.js';

// import { mat3 } from './gl-matrix-min.js';

const { mat3 } = glMatrix;

const gfa_path = "./data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa";
const layout_path = "./data/A-3105.layout.tsv";
const path_names = undefined;

// const gfa_path = "./MHC/HPRCy1v2.MHC.fa.ce6f12f.417fcdf.0ead406.smooth.final.gfa";
// const layout_path = "./MHC/HPRCy1v2.MHC.fa.ce6f12f.417fcdf.0ead406.smooth.final.og.lay.tsv";
// const path_names = [
//     "chm13#chr6:28385000-33300000",
//     "grch38#chr6:28510128-33480000",
//     "HG00438#1#h1tg000040l:22870040-27725000",
//     "HG00673#2#h2tg000031l:10256-1959976",
//     "HG00733#2#h2tg000060l:26405000-27483925",
//     "HG01175#1#h1tg000188l:192-200000",
//     "HG02818#2#h2tg000045l:15000-2706770",
//     "HG03516#2#h2tg000202l:24-441470",
// ];

function globalSequenceTrack(graph, canvas, view_subject) {

    const min_px_per_bp = 8.0;
    const seq_array = graph.segment_sequences_array();

    view_subject.pipe(
        rxjs.distinct(),
        rxjs.throttleTime(10)
    ).subscribe((view) => {
        requestAnimationFrame((time) => {
            let view_len = view.end - view.start;
            let px_per_bp = canvas.width / view_len;
            let ctx = canvas.getContext('2d');
            ctx.clearRect(0, 0, canvas.width, canvas.height);

            if (px_per_bp > min_px_per_bp) {
                let seq = seq_array.slice(view.start, view.end);
                let subpixel_offset = view.start - Math.trunc(view.start);
                CanvasTracks.drawSequence(canvas, seq, subpixel_offset);
            }
        });
    });
}

function addTestOverlay() {
    let data_canvas = document.getElementById('viewer-gi|568815551:1197321-1201446');
    console.log(data_canvas);
    let path_viewer = data_canvas.path_viewer;
    console.log(path_viewer);

    let entries = [{ start: 100, end: 1000, color: 'red' },
                   { start: 2100, end: 5000, color: 'blue' },
                  ];

    let callback = CanvasTracks.createHighlightCallback(entries);

    path_viewer.trackCallbacks['test'] = callback;
}

async function init() {

    const handler = await import('./transfer_handlers.js');
    handler.setTransferHandlers(rxjs, Comlink);

    const wasm = await init_module();

    const worker = new Worker("main_worker.js", { type: 'module' });

    worker.onmessage = async (event) => {

        if (event.data === "WORKER_INIT") {
            console.log("received from worker");
            worker.postMessage([wasm.memory, gfa_path]);
        } else if (event.data === "GRAPH_READY") {
            worker.onmessage = undefined;

            console.log("graph loaded");
            const worker_obj = Comlink.wrap(worker);

            const graph_raw = await worker_obj.getGraph();
            let graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph_raw.__wbg_ptr);

            window.addTestOverlay = addTestOverlay;

            // let segc = await worker_obj.getSegmentCount();
            // let segc = wrapped.segment_count();
            // console.log(" index.js >>>>>>>>>>>>>> " + segc);


            let cs_view = await worker_obj.globalCoordSysView();

            const graph_viewer = await initGraphViewer(wasm.memory, graph_raw, layout_path);
            console.log(graph_viewer);

            let view_subject = await cs_view.viewSubject();

            globalSequenceTrack(
                graph,
                document.getElementById('sequence-view'),
                view_subject
            );


            /*
            {
                let path_name = "gi|157734152:29655295-29712160";
                let path_range = await worker_obj.pathRangeToStepRange(path_name, 100n, 4000n);
                let path_steps = graph.path_steps(path_name);
                let path_slice = path_steps.slice(path_range.start, path_range.end);

                graph_viewer.overlayCallbacks['test'] = (canvas, view) => {
                    console.log(view);
                    let path2d = graph_viewer
                        .segment_positions
                        .path_to_canvas_space(view, canvas.width, canvas.height, path_slice);

                    let ctx = canvas.getContext('2d');
                    ctx.save();
                    ctx.globalAlpha = 0.8;
                    ctx.globalCompositeOperation = "copy";
                    ctx.lineWidth = 15;
                    ctx.strokeStyle = 'black';
                    ctx.stroke(path2d);
                    ctx.lineWidth = 10;
                    ctx.strokeStyle = 'red';
                    ctx.stroke(path2d);
                    ctx.restore();

                };

            }
            */


            window.graph_viewer = graph_viewer;

            console.log(" getting the view: ");
            console.log(graph_viewer.get_view_matrix());


            let names;
            if (path_names) {
                names = path_names;
            } else {
                names = await worker_obj.getPathNames();
            }

            console.log(names);

            // let path_range = await worker_obj.pathRangeToStepRange("gi|568815551:1197321-1201446", 100, 600);
            // console.log(path_range);

            // let path_cs_ptr = await worker_obj.pathCoordSys("gi|568815551:1197321-1201446");
            // console.log(path_cs);

            /*
            window.print_view_seq = async () => {
                let { start, end, len } = await cs_view.get();
                if (len < 400) {
                    let seq = seq_array.slice(start, end);
                    let el = document.getElementById("overlay-gi|568815551:1197321-1201446");
                    let subpixel_offset = start - Math.trunc(start);
                    CanvasTracks.drawSequence(el, seq, subpixel_offset);
                } else {
                    console.log("long sequence");
                }
            };
            */

            let view_max = await cs_view.viewMax();

            let overview_el = document.getElementById('overview-map');
            let overview = new OverviewMap(overview_el,  view_max);
            await addOverviewEventHandlers(overview, cs_view);

            let container = document.getElementById('path-viewer-container');

            names.forEach(async (name, path_ix) => {

                console.log("path: " + name);

                // let { path_viewer, canvas } =
                let path_viewer =
                    await initializePathViewer(worker_obj,
                                               overview,
                                               cs_view,
                                               name);

                path_viewer.canvas.path_viewer = path_viewer;

                const row_el = document.createElement("div");
                row_el.classList.add("path-viewer-list-row")

                const name_el = document.createElement("div");
                name_el.classList.add("path-name");
                name_el.innerHTML = name;

                let id = "viewer-" + name;
                path_viewer.canvas.classList.add("path-data-view");

                path_viewer.canvas.id = id;
                row_el.append(name_el);
                row_el.append(path_viewer.canvas);

                let overlay_el = document.createElement("canvas");
                overlay_el.classList.add("path-data-overlay");
                overlay_el.id = "overlay-" + name;

                path_viewer.overlay_canvas = overlay_el;

                addPathViewerLogic(worker_obj, path_viewer, overview_el, cs_view);

                row_el.append(overlay_el);

                container.append(row_el);

                await path_viewer.worker_ctx.setCanvasWidth(overlay_el.clientWidth);
                overlay_el.width = overlay_el.clientWidth;
                overlay_el.height = 40;

                console.log(path_viewer.canvas);
                console.log(path_viewer.canvas.clientWidth);

                if (path_ix == 0) {
                    overview_el.width = overlay_el.clientWidth;

                    let seq_view_el = document.getElementById('sequence-view');
                    seq_view_el.width = overlay_el.clientWidth;
                }

            });


            window.worker_obj = worker_obj;
        }

        
    };

}

init();
