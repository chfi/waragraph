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

// import * as FloatingUI from 'https://cdn.jsdelivr.net/npm/@floating-ui/dom@1.5.3/+esm';

// import { mat3 } from './gl-matrix-min.js';

const { mat3 } = glMatrix;

const gfa_path = "./data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa";
const layout_path = "./data/A-3105.layout.tsv";
const path_names = undefined;

// const path_names = ["gi|568815592:29942469-29945883"];

// const gfa_path = "./MHC/HPRCy1v2.MHC.fa.ce6f12f.417fcdf.0ead406.smooth.final.gfa";
// const layout_path = "./MHC/HPRCy1v2.MHC.fa.ce6f12f.417fcdf.0ead406.smooth.final.og.lay.tsv";
// const path_names = [
//     "chm13#chr6:28385000-33300000",
//     "grch38#chr6:28510128-33480000",
//     "HG02717#2#h2tg000061l:22650152-27715000",
//     "HG03516#1#h1tg000073l:22631064-27570000",
//     "HG00733#1#h1tg000070l:28540000-33419448",
//     "HG02055#1#h1tg000074l:0-4714592",
//     "HG01978#1#h1tg000035l:28455000-33469848",
//     "HG02886#2#h2tg000003l:25120800-30214744",
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

const TEST_BED = [
    { path: "gi|568815569:1240288-1243708", start: 100, end: 3000, label: "TEST" },
];


const MHC_BED = [
    { path: "grch38#chr6", start: 29722775, end: 29738528, label: "HLA-F" },
    { path: "grch38#chr6", start: 29726601, end: 29749049, label: "HLA-F-AS1" },
    { path: "grch38#chr6", start: 29790954, end: 29797811, label: "HLA-V" },
    { path: "grch38#chr6", start: 29800415, end: 29802425, label: "HLA-P" },
    { path: "grch38#chr6", start: 29826967, end: 29831125, label: "HLA-G" },
    { path: "grch38#chr6", start: 29887752, end: 29890482, label: "HLA-H" },
    { path: "grch38#chr6", start: 29896654, end: 29897786, label: "HLA-T" },
    { path: "grch38#chr6", start: 29926459, end: 29929232, label: "HLA-K" },
    { path: "grch38#chr6", start: 29934101, end: 29934286, label: "HLA-U" },
    { path: "grch38#chr6", start: 29941260, end: 29945884, label: "HLA-A" },
    { path: "grch38#chr6", start: 29956596, end: 29958570, label: "HLA-W" },
    { path: "grch38#chr6", start: 30005971, end: 30009956, label: "HLA-J" },
    { path: "grch38#chr6", start: 30259548, end: 30293015, label: "HLA-L" },
    { path: "grch38#chr6", start: 30351416, end: 30351550, label: "HLA-N" },
    { path: "grch38#chr6", start: 30489509, end: 30494194, label: "HLA-E" },
    { path: "grch38#chr6", start: 31268749, end: 31272130, label: "HLA-C" },
    { path: "grch38#chr6", start: 31353872, end: 31367067, label: "HLA-B" },
    { path: "grch38#chr6", start: 31382074, end: 31382288, label: "HLA-S" },
    { path: "grch38#chr6", start: 32439878, end: 32445046, label: "HLA-DRA" },
    { path: "grch38#chr6", start: 32459821, end: 32473500, label: "HLA-DRB9" },
    { path: "grch38#chr6", start: 32517353, end: 32530287, label: "HLA-DRB5" },
    { path: "grch38#chr6", start: 32552713, end: 32560022, label: "HLA-DRB6" },
    { path: "grch38#chr6", start: 32577902, end: 32589848, label: "HLA-DRB1" },
    { path: "grch38#chr6", start: 32628179, end: 32647062, label: "HLA-DQA1" },
    { path: "grch38#chr6", start: 32659467, end: 32668383, label: "HLA-DQB1" },
    { path: "grch38#chr6", start: 32659880, end: 32660729, label: "HLA-DQB1-AS1" },
    { path: "grch38#chr6", start: 32730758, end: 32731695, label: "HLA-DQB3" },
    { path: "grch38#chr6", start: 32741391, end: 32747198, label: "HLA-DQA2" },
    { path: "grch38#chr6", start: 32756098, end: 32763532, label: "HLA-DQB2" },
    { path: "grch38#chr6", start: 32812763, end: 32820466, label: "HLA-DOB" },
    { path: "grch38#chr6", start: 32896416, end: 32896490, label: "HLA-Z" },
    { path: "grch38#chr6", start: 32934629, end: 32941028, label: "HLA-DMB" },
    { path: "grch38#chr6", start: 32948613, end: 32969094, label: "HLA-DMA" },
    { path: "grch38#chr6", start: 33004182, end: 33009591, label: "HLA-DOA" },
    { path: "grch38#chr6", start: 33064569, end: 33080775, label: "HLA-DPA1" },
    { path: "grch38#chr6", start: 33091753, end: 33097295, label: "HLA-DPA2" },
    { path: "grch38#chr6", start: 33112440, end: 33129084, label: "HLA-DPB2" },
    { path: "grch38#chr6", start: 33131216, end: 33143325, label: "HLA-DPA3" },
    { path: "grch38#chr6", start: 33075990, end: 33089696, label: "HLA-DPB1" },
]

function getRandomColor() {
    const r = Math.floor(Math.random() * 256);
    const g = Math.floor(Math.random() * 256);
    const b = Math.floor(Math.random() * 256);
    return `rgb(${r},${g},${b})`;
}

async function addTestOverlay(graph, worker_obj, graph_viewer) {
    // let path_name = 'gi|568815551:1197321-1201446';
    let path_name = "gi|568815569:1240288-1243708";
    // let path_name = 'grch38#chr6:28510128-33480000';

    let data_canvas = document.getElementById('viewer-' + path_name);
    let path_viewer = data_canvas.path_viewer;

    const path_steps = graph.path_steps(path_name);

    // these are global coordinates
    // let path_entries = [{ start: 100, end: 1000, color: 'blue', label: 'AAAA' },
    //                     { start: 2100, end: 5000, color: 'blue', label: 'BBBB' },
    // let path_entries = [{ start: 100, end: 1000, color: 'blue', label: 'AAAA' },
    //                     { start: 2100, end: 5000, color: 'blue', label: 'BBBB' },
    //               ];


    const cs_view = await worker_obj.globalCoordSysView();

    const path_offset = 0;
    let path_entries = TEST_BED.map((row) => {

        let color = wasm_bindgen.path_name_hash_color_obj(row.label);
        console.log(color);

        return { start: row.start - path_offset,
                 end: row.end - path_offset,
                 color: `rgb(${color.r * 255}, ${color.g * 255}, ${color.b * 255})`,
                 label: row.label };
    });


    let global_entries = [];

    for (const path_entry of path_entries) {
        try {
        let path_range = await worker_obj.pathRangeToStepRange(path_name, path_entry.start, path_entry.end);
        let slice = path_steps.slice(path_range.start, path_range.end);

        let seg_ranges = wasm_bindgen.path_slice_to_global_adj_partitions(slice);
        let seg_ranges_arr = seg_ranges.ranges_as_u32_array();
        let range_count = seg_ranges_arr.length / 2;

        for (let ri = 0; ri < range_count; ri++) {
            let start_seg = seg_ranges_arr.at(2 * ri);
            let end_seg = seg_ranges_arr.at(2 * ri + 1);

            if (start_seg && end_seg) {
                let start = await cs_view.segmentOffset(start_seg);
                let end = await cs_view.segmentOffset(end_seg);

                global_entries.push({start, end, color: path_entry.color});
            }
        }
        } catch (e) {
            //
        }
    }

    console.log(global_entries.length);

    let callback = CanvasTracks.createHighlightCallback(global_entries);

    path_viewer.trackCallbacks['test'] = callback;

    let path_cs = await worker_obj.pathCoordSys(path_name);

    let callback_2d =
        preparePathHighlightOverlay(graph_viewer.segment_positions,
                                    path_steps,
                                    path_cs,
                                    path_entries);

    graph_viewer.overlayCallbacks['test'] = callback_2d;
    console.log("?????");

    // for (const entry of path_entries) {
    //     let callback_2d =
    //         preparePathHighlightOverlay(graph_viewer.segment_positions,
    //                                     path_steps,
    //                                     path_cs,
    //                                     [entry]);

    //     graph_viewer.overlayCallbacks[entry.label] = callback_2d;
    // }

}


async function addViewRangeInputListeners(cs_view) {
    const start_el = document.getElementById('path-viewer-range-start');
    const end_el = document.getElementById('path-viewer-range-end');

    let init_view = await cs_view.get();

    start_el.value = init_view.start;
    end_el.value = init_view.end;

    start_el.addEventListener('change', (event) => {
        cs_view.set({ start: start_el.value, end: end_el.value });
    });

    end_el.addEventListener('change', (event) => {
        cs_view.set({ start: start_el.value, end: end_el.value });
    });

    const view_subject = await cs_view.viewSubject();

    view_subject.subscribe((view) => {
        start_el.value = Math.round(view.start);
        end_el.value = Math.round(view.end);
    });
}

async function init() {

    const handler = await import('./transfer_handlers.js');
    handler.setTransferHandlers(rxjs, Comlink);

    const wasm = await init_module();

    // const worker = new Worker("main_worker.js", { type: 'module' });
    const worker = new Worker(new URL("main_worker.js", import.meta.url), { type: 'module' });

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

            console.log(" >>>>>>>> segment ranges");
            for (let i = 0; i < 10; i++) {
                let _range = await cs_view.segmentRange(i);
                console.log(i + ": " + _range.start + "-" + _range.end + ", length: " + (_range.end - _range.start));
                // console.log(_range);
            }

            addViewRangeInputListeners(cs_view);

            const graph_viewer = await initGraphViewer(wasm.memory, graph_raw, layout_path);
            console.log(graph_viewer);


            window.addTestOverlay = () => {
                addTestOverlay(graph, worker_obj, graph_viewer);
            };

            let view_subject = await cs_view.viewSubject();

            
            {
                let state = false;
            document.getElementById('view-toggle').addEventListener('click', (ev) => {

                if (state) {
                    state = false;
                    cs_view.set({ start: 618, end: 4460 });
                } else {
                    state = true;
                    cs_view.set({ start: 638, end: 4442 });
                }

            });
            }

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
