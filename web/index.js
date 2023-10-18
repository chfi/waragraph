import * as Comlink from './comlink.mjs';

const { Observable } = rxjs;

import { initializePathViewer, addOverviewEventHandlers, addPathViewerLogic } from './path_viewer_ui.js';

import init_module, * as wasm_bindgen from './pkg/web.js';

import { OverviewMap } from './overview.js';

import { GraphViewer, initGraphViewer } from './graph_viewer.js';

import * as CanvasTracks from './canvas_tracks.js';

import { mat3 } from './gl-matrix-min.js';

const gfa_path = "./data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa";
const layout_path = "./data/A-3105.layout.tsv";

// const gfa_path = "./MHC/HPRCy1v2.MHC.fa.ce6f12f.417fcdf.0ead406.smooth.final.gfa";
// const layout_path = "./MHC/HPRCy1v2.MHC.fa.ce6f12f.417fcdf.0ead406.smooth.final.og.lay.tsv";

const path_names = undefined;
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

async function init() {

    const handler = await import('./transfer_handlers.js');
    handler.setTransferHandlers(rxjs, Comlink);

    const wasm = await init_module();

    const worker = new Worker("main_worker.js", { type: 'module' });

    worker.onmessage = async (event) => {

        if (event.data === "WORKER_INIT") {
            console.log("received from worker");
            console.log(event.data);
            console.log(wasm.memory);
            worker.postMessage([wasm.memory, gfa_path]);
        } else if (event.data === "GRAPH_READY") {
            worker.onmessage = undefined;

            console.log("graph loaded");
            const worker_obj = Comlink.wrap(worker);

            const graph = await worker_obj.getGraph();

            // let segc = await worker_obj.getSegmentCount();
            // let segc = wrapped.segment_count();
            // console.log(" index.js >>>>>>>>>>>>>> " + segc);


            const graph_viewer = await initGraphViewer(wasm.memory, graph, layout_path);

            let names;
            if (path_names) {
                names = path_names;
            } else {
                names = await worker_obj.getPathNames();
            }

            console.log(names);

            let cs_view = await worker_obj.globalCoordSys();

            let view_max = await cs_view.viewMax();

            let overview_el = document.getElementById('overview-map');
            let overview = new OverviewMap(overview_el,  view_max);
            await addOverviewEventHandlers(overview, cs_view);

            let container = document.getElementById('container');

            names.forEach(async (name, path_ix) => {

                console.log("path: " + name);

                let { path_viewer, canvas } =
                    await initializePathViewer(worker_obj,
                                               overview,
                                               cs_view,
                                               name);


                const row_el = document.createElement("div");
                row_el.classList.add("path-viewer-list-row")

                const name_el = document.createElement("div");
                name_el.classList.add("path-name");
                name_el.innerHTML = name;

                let id = "viewer-" + name;
                canvas.classList.add("path-data-view");

                canvas.id = id;
                // name_container.append(name_el);
                // data_container.append(canvas);
                row_el.append(name_el);
                row_el.append(canvas);

                let overlay_el = document.createElement("canvas");
                overlay_el.classList.add("path-data-overlay");
                overlay_el.id = "overlay-" + name;
                overlay_el.width = 1024;
                overlay_el.height = 40;

                addPathViewerLogic(worker_obj, path_viewer, overlay_el, overview_el, cs_view);

                row_el.append(overlay_el);

                container.append(row_el);

                console.log(canvas);
                console.log(canvas.clientWidth);

                if (path_ix == 0) {
                    overview_el.width = canvas.clientWidth;
                }

                if (path_ix == 1) {
                    // let ranges = [
                    //     { start: 100.0, end: 120.0, thickness: 12.0 },
                    //     { start: 120.0, end: 180.0, thickness: 4.0 },
                    //     { start: 180.0, end: 230.0, thickness: 12.0 },
                    // ];
                    // CanvasTracks.drawVariableThicknessTrack(overlay_el, ranges);

                    let ranges = [
                        { start: 100.0, end: 120.0, thick: true },
                        { start: 120.0, end: 180.0, thick: false },
                        { start: 180.0, end: 230.0, thick: true },
                    ];
                    CanvasTracks.drawBinaryArrowTrack(overlay_el, ranges);
                }

                // let parent_w = data_container.clientWidth;
                // console.log(parent_w);
                // await path_viewer.setCanvasWidth(parent_w);

            });

            // div.path-viewer-list-row:nth-child(2) > div:nth-child(1)

            // let col2 = document.querySelector("canvas.path-data-view");
            // console.log(col2);


            window.worker_obj = worker_obj;
        }

        
    };

}

init();
