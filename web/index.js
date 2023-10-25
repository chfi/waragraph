import * as Comlink from './comlink.mjs';

const { Observable } = rxjs;

import { initializePathViewer, addOverviewEventHandlers, addPathViewerLogic } from './path_viewer_ui.js';

import init_module, * as wasm_bindgen from './pkg/web.js';

import { OverviewMap } from './overview.js';

import { GraphViewer,
         initGraphViewer,
         preparePathHighlightOverlay
       } from './graph_viewer.js';

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




async function addTestOverlay(graph, worker_obj, graph_viewer) {
    let path_name = 'gi|568815551:1197321-1201446';

    let data_canvas = document.getElementById('viewer-' + path_name);
    let path_viewer = data_canvas.path_viewer;

    const path_steps = graph.path_steps(path_name);

    // these are global coordinates
    let path_entries = [{ start: 100, end: 1000, color: 'red' },
                   { start: 2100, end: 5000, color: 'red' },
                  ];

    let global_entries = [];

    path_entries.forEach((path_entry) => {
        let path_range = worker_obj.pathRangeToStepRange(path_name, path_entry.start, path_entry.end);
        let slice = path_steps.slice(path_range.start, path_range.end);
        let seg_ranges = wasm_bindgen.path_slice_to_global_adj_partitions(slice);
        let seg_ranges_arr = seg_ranges.ranges_as_u32_array();
        let range_count = seg_ranges_arr.length / 2;

        for (let ri = 0; ri < range_count; ri++) {
            let start = seg_ranges_arr.at(2 * ri);
            let end = seg_ranges_arr.at(2 * ri + 1);
            global_entries.push({start, end, color: path_entry.color});
        }
        
    });

    let callback = CanvasTracks.createHighlightCallback(global_entries);

    path_viewer.trackCallbacks['test'] = callback;

    let path_cs = await worker_obj.pathCoordSys(path_name);

    let callback_2d =
        preparePathHighlightOverlay(graph_viewer.segment_positions,
                                    path_steps,
                                    path_cs,
                                    path_entries);

    graph_viewer.overlayCallbacks['test'] = callback_2d;
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

            // window.addTestOverlay = addTestOverlay;

            let cs_view = await worker_obj.globalCoordSysView();

            const graph_viewer = await initGraphViewer(wasm.memory, graph_raw, layout_path);
            console.log(graph_viewer);


            window.addTestOverlay = () => {
                addTestOverlay(graph, worker_obj, graph_viewer);
            };

            let view_subject = await cs_view.viewSubject();

            globalSequenceTrack(
                graph,
                document.getElementById('sequence-view'),
                view_subject
            );

            window.graph_viewer = graph_viewer;

            console.log(graph_viewer.get_view_matrix());

            let names;
            if (path_names) {
                names = path_names;
            } else {
                names = await worker_obj.getPathNames();
            }

            console.log(names);

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
