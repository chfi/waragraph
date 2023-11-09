import init_module, * as wasm_bindgen from '../pkg/web.js';
import BED from '@gmod/bed';

import { preparePathHighlightOverlay } from '../graph_viewer.js';

import '../sidebar-bed.css';

let pathNamesMap = new Map();

function findPathName(cand) {
    if (pathNamesMap.has(cand)) {
        return pathNamesMap.get(cand);
    }

    for (const path_name of pathNamesMap.keys()) {
        if (path_name.startsWith(cand)) {
            return path_name;
        }
    }
}


let waragraph_viz = null;
let wasm = null;

function createPathOffsetMap(path_name) {
    const regex = /.+:(\d+)-(\d+)$/;
    const found = path_name.match(regex);

    if (found === null) {
        return (x) => x;
    }

    const start = Number(found[1]);

    return (bp) => bp - start;
}

function transformBedRange(bed_entry) {
    let name = bed_entry.chrom;

    const regex = /.+:(\d+)-(\d+)$/;

    const found = name.match(regex);

    if (found === null) {
        return bed_entry;
    }

    const start = Number(found[1]);

    const chromStart = bed_entry.chromStart - start;
    const chromEnd = bed_entry.chromEnd - start;

    const new_entry = Object.assign({}, bed_entry, { chromStart, chromEnd });

    return new_entry;
}

async function createDrawBedEntryFn(bed_entry) {
    let path_name = findPathName(bed_entry.chrom);
    let path_offset_map = createPathOffsetMap(path_name);

    let seg_pos = waragraph_viz.graph_viewer.segment_positions;

    let graph_raw = await waragraph_viz.worker_obj.getGraph();
    let graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph_raw.__wbg_ptr);
    let path_steps = graph.path_steps(path_name);

    let path_cs = await waragraph_viz.worker_obj.pathCoordSys(path_name);

    let entry = { start: path_offset_map(bed_entry.chromStart),
                  end: path_offset_map(bed_entry.chromEnd),
                  label: bed_entry.name };

    if (typeof bed_entry.itemRgb === "string") {
        let [r,g,b] = bed_entry.split(',');
        entry.color = `rgb(${r * 255},${g * 255},${b * 255})`;
    } else if (bed_entry.color) {
        entry.color = bed_entry.color;
    } else {
        let {r,g,b} = wasm_bindgen.path_name_hash_color_obj(entry.label);
        entry.color = `rgb(${r * 255},${g * 255},${b * 255})`;
    }

    let callback_2d = 
        preparePathHighlightOverlay(seg_pos,
                                    path_steps,
                                    path_cs,
                                    [entry]);

    return callback_2d;
}
                            

async function loadBedFile(file) {
    const bed_list = document.getElementById('bed-file-list');

    const entries_list = document.createElement('div');
    entries_list.classList.add('bed-file-entry');

    const name_el = document.createElement('div');
    name_el.innerHTML = file.name;
    name_el.classList.add('bed-file-name');
    name_el.style.setProperty('flex-basis', '30px');

    entries_list.append(name_el);

    bed_list.append(entries_list);

    const bed_text = await file.text();

    const parser = new BED();

    const bed_lines = bed_text.split('\n').map(line => parser.parseLine(line));

    for (const bed_entry of bed_lines) {

        if (!Number.isNaN(bed_entry.chromStart)
            && !Number.isNaN(bed_entry.chromEnd)) {
            const entry = transformBedRange(bed_entry);
            console.log(entry);

            const draw_bed = await createDrawBedEntryFn(entry);
            console.log(draw_bed);

            const cb_key = file.name + ":" + entry.name;

            let active = false;

            const entry_div = document.createElement('div');
            entry_div.classList.add('bed-file-row');
            // entry_div.style.setProperty('flex-basis', '20px');

            const label_div = document.createElement('div');
            label_div.innerHTML = entry.name;
            label_div.classList.add('bed-row-label');

            const tgl_1d = document.createElement('button');
            tgl_1d.innerHTML = "1D";
            tgl_1d.classList.add('bed-row-1d', 'highlight-disabled');
            tgl_1d.style.setProperty('display', 'block');

            const tgl_2d = document.createElement('button');
            tgl_2d.innerHTML = "2D";
            tgl_2d.classList.add('bed-row-2d', 'highlight-disabled');
            tgl_2d.style.setProperty('display', 'block');

            entry_div.append(label_div);
            entry_div.append(tgl_1d);
            entry_div.append(tgl_2d);

            const toggle_highlight_class = (el) => {
                if (el.classList.contains('highlight-enabled')) {
                    el.classList.remove('highlight-enabled');
                    el.classList.add('highlight-disabled');
                    return false;
                } else {
                    el.classList.remove('highlight-disabled');
                    el.classList.add('highlight-enabled');
                    return true;
                }
            };

            tgl_1d.addEventListener('click', (e) => {
                if (toggle_highlight_class(tgl_1d)) {
                    // TODO add callback
                } else {
                    // TODO remove callback
                }
            });

            tgl_2d.addEventListener('click', (e) => {
                if (toggle_highlight_class(tgl_2d)) {
                    waragraph_viz.graph_viewer.registerOverlayCallback(cb_key, draw_bed);
                } else {
                    waragraph_viz.graph_viewer.removeOverlayCallback(cb_key);
                }
            });

            entries_list.append(entry_div);
        }

    }

}

function bedSidebarPanel() {
    const bed_pane = document.createElement('div');
    bed_pane.classList.add('bed-panel');

    const bed_list = document.createElement('div');
    bed_list.id = 'bed-file-list';

    bed_pane.append(bed_list);

    const file_label = document.createElement('label');
    file_label.setAttribute('for', 'bed-file-input');
    file_label.innerHTML = 'Load BED file';

    const file_entry = document.createElement('input');
    file_entry.id = 'bed-file-input';
    file_entry.setAttribute('type', 'file');
    file_entry.setAttribute('name', 'bed-file-input');
    file_entry.setAttribute('accept', '.bed');

    const file_button = document.createElement('button');
    file_button.innerHTML = 'Load';

    file_button.addEventListener('click', (ev) => {
        for (const file of file_entry.files) {
            loadBedFile(file);
        }
    });

    bed_pane.append(file_label);
    bed_pane.append(file_entry);
    bed_pane.append(file_button);


    return bed_pane;
}


export async function initializeBedSidebarPanel(warapi) {
    waragraph_viz = warapi;

    wasm = await init_module(undefined, waragraph_viz.wasm.memory);
    wasm_bindgen.set_panic_hook();

    let path_names = await warapi.worker_obj.getPathNames();
    let path_index = 0;

    for (const name of path_names) {
        pathNamesMap.set(name, path_index);
        path_index += 1;
    }

    document
        .getElementById('sidebar')
        .append(bedSidebarPanel());
}
