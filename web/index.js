import * as Comlink from 'comlink';
import { Observable } from 'rxjs';
import * as rxjs from 'rxjs';

import * as handler from './transfer_handlers.js';
handler.setTransferHandlers(rxjs, Comlink);

import Split from 'split-grid';

import { initializePathViewer, addOverviewEventHandlers, addPathViewerLogic } from './path_viewer_ui.js';

import init_module, * as wasm_bindgen from './pkg/web.js';

import { OverviewMap } from './overview.js';

import { GraphViewer,
         initGraphViewer,
         initializeGraphViewer,
         preparePathHighlightOverlay
       } from './graph_viewer.js';

import * as CanvasTracks from './canvas_tracks.js';


import * as BedSidebar from './js/sidebar-bed.js';

// const gfa_path = "./data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa";
// const layout_path = "./data/A-3105.layout.tsv";
// const path_names = undefined;

// const path_names = ["gi|568815592:29942469-29945883"];

const gfa_path = "./MHC/HPRCy1v2.MHC.fa.ce6f12f.417fcdf.0ead406.smooth.final.gfa";
const layout_path = "./MHC/HPRCy1v2.MHC.fa.ce6f12f.417fcdf.0ead406.smooth.final.og.lay.tsv";
const path_names = [
    "chm13#chr6:28385000-33300000",
    "grch38#chr6:28510128-33480000",
    "HG02717#2#h2tg000061l:22650152-27715000",
    "HG03516#1#h1tg000073l:22631064-27570000",
    "HG00733#1#h1tg000070l:28540000-33419448",
    "HG02055#1#h1tg000074l:0-4714592",
    "HG01978#1#h1tg000035l:28455000-33469848",
    "HG02886#2#h2tg000003l:25120800-30214744",
];

function globalSequenceTrack(graph, canvas, view_subject) {

    const min_px_per_bp = 8.0;
    const seq_array = graph.segment_sequences_array();

    let last_view = null;

    const draw_view = (view) => {
        let view_len = view.end - view.start;
        let px_per_bp = canvas.width / view_len;
        let ctx = canvas.getContext('2d');
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        if (px_per_bp > min_px_per_bp) {
            let seq = seq_array.slice(view.start, view.end);
            let subpixel_offset = view.start - Math.trunc(view.start);
            CanvasTracks.drawSequence(canvas, seq, subpixel_offset);

            last_view = view;
        }
    };

    view_subject.pipe(
        rxjs.distinct(),
        rxjs.throttleTime(10)
    ).subscribe((view) => {
        requestAnimationFrame((time) => {
            draw_view(view);
        });
    });

    const draw_last = () => {
        if (last_view !== null) {
            draw_view(last_view);
        }
    };

    return { draw_last };
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



function appendPathListElements(height, left_tag, right_tag) {
    const left = document.createElement(left_tag);
    const right = document.createElement(right_tag);

    const setStyles = (el) => {
        el.style.setProperty("flex-basis", height + "px");
    };

    setStyles(left);
    setStyles(right);

    document.getElementById("path-viewer-left-column").append(left);
    document.getElementById("path-viewer-right-column").append(right);

    return { left, right };
}

async function appendPathView(worker_obj, resize_subject, path_name) {


    const name_column = document.getElementById('path-viewer-left-column');
    const data_column = document.getElementById('path-viewer-right-column');

    const name_el = document.createElement('div');
    const data_el = document.createElement('div');

    name_el.classList.add('path-list-flex-item', 'path-name');
    data_el.classList.add('path-list-flex-item');

    name_el.innerHTML = path_name;

    let cs_view = await worker_obj.globalCoordSysView();

    name_column.append(name_el);
    data_column.append(data_el);

    let path_viewer = await initializePathViewer(worker_obj,
                                                 cs_view,
                                                 path_name,
                                                 data_el,
                                                resize_subject);


    addPathViewerLogic(worker_obj, path_viewer, cs_view, resize_subject);

}

class WaragraphViz {
    constructor(
        wasm,
        worker_obj,
                graph_viewer,
               ) {
        this.wasm = wasm;
        this.worker_obj = worker_obj;
        this.graph_viewer = graph_viewer;
    }

    // TODO API for interfacing with graph and viewers/views here
                
}

const init = async () => {
    const wasm = await init_module();
    const worker = new Worker(new URL("main_worker.js", import.meta.url), { type: 'module' });

    window.wasm_bindgen = wasm;

    worker.onmessage = async (event) => {
        if (event.data === "WORKER_INIT") {
            worker.postMessage([wasm.memory, gfa_path]);
        } else if (event.data === "GRAPH_READY") {
            worker.onmessage = undefined;

            const worker_obj = Comlink.wrap(worker);

            const graph_raw = await worker_obj.getGraph();

            const graph_viewer = await initializeGraphViewer(wasm.memory, graph_raw, layout_path);

            const warapi = new WaragraphViz(wasm, worker_obj, graph_viewer);

            window.getPathCoordSys = async (path_name) => {
                return await worker_obj.pathCoordSys(path_name);
            };

            // getPathRange("grch38#chr6:28510128-33480000", 1841288n, 1841422n)
            window.getPathRange = async (path_name, start, end) => {
                let cs_raw = await worker_obj.pathCoordSys(path_name);
                let cs = wasm_bindgen.CoordSys.__wrap(cs_raw.__wbg_ptr);
                return cs.bp_to_step_range(start, end);
            };

            await BedSidebar.initializeBedSidebarPanel(warapi);

            const resize_obs = new rxjs.Subject();

            let names;
            if (path_names) {
                names = path_names;
            } else {
                names = await worker_obj.getPathNames();
            }

            {
                // TODO: factor out overview & range input bits
                const overview_slots = appendPathListElements(40, 'div', 'div');

                const cs_view = await worker_obj.globalCoordSysView();
                const view_max = await cs_view.viewMax();
                // const view_subject = await cs_view.viewSubject();
                const overview_canvas = document.createElement('canvas');
                overview_canvas.style.setProperty('position', 'absolute');
                overview_canvas.style.setProperty('overflow', 'hidden');
                overview_canvas.width = overview_slots.right.clientWidth;
                overview_canvas.height = overview_slots.right.clientHeight;
                overview_slots.right.append(overview_canvas);
                const overview = new OverviewMap(overview_canvas, view_max);
                await addOverviewEventHandlers(overview, cs_view);

                // range input
                const range_input = document.createElement('div');
                // range_input.classList.add('path-name');
                range_input.id = 'path-viewer-range-input';

                overview_slots.left.append(range_input);

                for (const id of ["path-viewer-range-start", "path-viewer-range-end"]) {
                    const input = document.createElement('input');
                    input.id = id;
                    input.setAttribute('type', 'text');
                    input.setAttribute('inputmode', 'numeric');
                    input.setAttribute('pattern', '\d*');
                    input.style.setProperty('height', '100%');
                    range_input.append(input);
                }

                await addViewRangeInputListeners(cs_view);

                // TODO: factor out sequence track bit maybe

                const seq_slots = appendPathListElements(20, 'div', 'div');

                const seq_canvas = document.createElement('canvas');
                seq_canvas.width = seq_slots.right.clientWidth;
                seq_canvas.height = seq_slots.right.clientHeight;
                seq_canvas.style.setProperty('position', 'absolute');
                seq_canvas.style.setProperty('overflow', 'hidden');

                seq_slots.right.append(seq_canvas);

                let view_subject = await cs_view.viewSubject();

                let graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph_raw.__wbg_ptr);
                let seq_track = globalSequenceTrack(
                    graph,
                    seq_canvas,
                    view_subject
                );

                resize_obs.subscribe(() => {
                    overview_canvas.width = overview_slots.right.clientWidth;
                    overview_canvas.height = overview_slots.right.clientHeight;
                    seq_canvas.width = seq_slots.right.clientWidth;
                    seq_canvas.height = seq_slots.right.clientHeight;

                    overview.draw();
                    seq_track.draw_last();
                });

            }

            for (const path_name of names) {
                appendPathView(worker_obj, resize_obs, path_name);
            }

            // TODO: additional tracks

            const split_root = Split({
                columnGutters: [{
                    track: 1,
                    element: document.querySelector('.gutter-column-sidebar'),
                }],
                onDragEnd: (dir, track) => {
                    graph_viewer.resize();
                    resize_obs.next();
                },
            });

            const split_viz = Split({
                rowGutters: [{
                    track: 1,
                    element: document.querySelector('.gutter-row-1')
                }],
                columnGutters: [{
                    track: 1,
                    element: document.querySelector('.gutter-column-1')
                }],
                rowMinSizes: { 0: 200 },
                onDragEnd: (dir, track) => {
                    if (dir === "row" && track === 1) {
                        // 2D view resize
                        graph_viewer.resize();
                    } else if (dir === "column" && track === 1) {
                        // 1D view resize
                        resize_obs.next();
                    }
                },
            });

            rxjs.fromEvent(window, 'resize').pipe(
                rxjs.throttleTime(100),
            ).subscribe(() => {
                graph_viewer.resize();
                resize_obs.next();
            });
        }
    };

};

onload = init;
