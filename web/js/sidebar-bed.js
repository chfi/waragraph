import init_module, * as wasm_bindgen from '../pkg/web.js';

import BED from '@gmod/bed';

import {computePosition} from '@floating-ui/dom';

import { preparePathHighlightOverlay } from '../graph_viewer.js';

import { AnnotationPainter } from './annotations.js';

import * as CanvasTracks from '../canvas_tracks.js';

import '../sidebar-bed.css';

let pathNamesMap = new Map();

let _context_menu_entry = null;

function findPathName(cand) {
    if (pathNamesMap.has(cand)) {
        return cand;
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
    if (!path_name) {
        throw `Path ${path_name} not found`;
    }
    const found = path_name.match(regex);

    const start = found === null ? 0 : Number(found[1]);

    return (bp) => bp - start;
}

function bedToPathRange(bed_entry, path_name) {
    if (!path_name) {
        throw `Path ${path_name} not found`;
    }

    const regex = /.+:(\d+)-(\d+)$/;

    const found = path_name.match(regex);

    if (found === null) {
        return bed_entry;
    }

    const start = Number(found[1]);

    let chromStart = bed_entry.chromStart - start;
    let chromEnd = bed_entry.chromEnd - start;

    if (chromStart < 0) {
        chromStart = 0;
    }

    return { start: chromStart, end: chromEnd };
}

function transformBedRange(bed_entry) {
    let name = bed_entry.chrom;

    let path_name = findPathName(name);

    if (!path_name) {
        throw `Path ${path_name} not found`;
    }

    const regex = /.+:(\d+)-(\d+)$/;

    const found = path_name.match(regex);

    if (found === null) {
        return bed_entry;
    }

    const start = Number(found[1]);

    let chromStart = bed_entry.chromStart - start;
    let chromEnd = bed_entry.chromEnd - start;

    if (chromStart < 0) {
        chromStart = 0;
    }

    // TODO pass coordinate max in here, maybe;
    // if (chromEnd > max) {
    // }

    const new_entry = Object.assign({}, bed_entry, { chromStart, chromEnd });

    return new_entry;
}

function bedEntryColorOrFn(bed_entry, color_fn) {
    let color;

    if (typeof bed_entry.itemRgb === "string") {
        let [r,g,b] = bed_entry.itemRgb.split(',');
        color = `rgb(${r * 255},${g * 255},${b * 255})`;
    } else if (bed_entry.color) {
        color = bed_entry.color;
    } else {
        let {r,g,b} = wasm_bindgen.path_name_hash_color_obj(bed_entry.name);
        color = `rgb(${r * 255},${g * 255},${b * 255})`;
    }

    if (typeof color_fn === 'function') {
        color = color_fn(bed_entry);
    }

    return color;
}

async function createDrawBedEntryFn1d(bed_entry, color_fn) {
    //
    let path_name = findPathName(bed_entry.chrom);
    let path_offset_map = createPathOffsetMap(path_name);

    let graph_raw = await waragraph_viz.worker_obj.getGraph();
    let graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph_raw.__wbg_ptr);
    let path_steps = graph.path_steps(path_name);

    // let color = bedEntryColorOrFn(bed_entry, color_fn);

    let entry = { start: bed_entry.chromStart,
                  end: bed_entry.chromEnd,
                  label: bed_entry.name };

    entry.color = bedEntryColorOrFn(bed_entry, color_fn);

    const cs_view = await waragraph_viz.worker_obj.globalCoordSysView();

    let global_entries = [];

    try {
        // console.warn(entry);
        let path_range = await waragraph_viz
            .worker_obj
            .pathRangeToStepRange(path_name, entry.start, entry.end);

        // console.warn(path_range);
        let slice = path_steps.slice(path_range.start, path_range.end);
        // console.warn(slice);

        // console.log("steps in ", bed_entry.name, ": ", slice.length);

        let seg_ranges = wasm_bindgen.path_slice_to_global_adj_partitions(slice);
        let seg_ranges_arr = seg_ranges.ranges_as_u32_array();
        let range_count = seg_ranges_arr.length / 2;

        for (let ri = 0; ri < range_count; ri++) {
            let start_seg = seg_ranges_arr.at(2 * ri);
            let end_seg = seg_ranges_arr.at(2 * ri + 1);

            if (start_seg && end_seg) {
                let start = await cs_view.segmentOffset(start_seg);
                let end = await cs_view.segmentOffset(end_seg);

                global_entries.push({start, end, color: entry.color});
            }
        }
    } catch (e) {
        // TODO
        throw "Error creating 1D highlight track: " + e;
    }

    let callback = CanvasTracks.createHighlightCallback(global_entries);

    return callback;
}

async function createDrawBedEntryFn2d(bed_entry, color_fn) {
    // console.log(bed_entry);
    let path_name = findPathName(bed_entry.chrom);
    // console.log(path_name);
    let path_offset_map = createPathOffsetMap(path_name);

    let seg_pos = waragraph_viz.graph_viewer.segment_positions;

    let graph_raw = await waragraph_viz.worker_obj.getGraph();
    let graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph_raw.__wbg_ptr);
    let path_steps = graph.path_steps(path_name);

    let path_cs = await waragraph_viz.worker_obj.pathCoordSys(path_name);

    let entry = { start: bed_entry.chromStart,
                  end: bed_entry.chromEnd,
                  label: bed_entry.name };

    entry.color = bedEntryColorOrFn(bed_entry, color_fn);

    let callback_2d = 
        preparePathHighlightOverlay(seg_pos,
                                    path_steps,
                                    path_cs,
                                    [entry]);

    return callback_2d;
}
                            

class BEDFile {
    // constructor(file_name, record_lines) {
    constructor(file_name) {
        this.file_name = file_name;
        this.records = [];
        this.records_viz_state = [];

        this.annotation_painter = null;

        // per record: 
        // { canvas_1d: Map<Path, bool>,
        //   canvas_2d: bool,
        //   svg_shared: svg element
    }

    async appendRecords(record_lines) {

        const graph_raw = await waragraph_viz.worker_obj.getGraph();
        const graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph_raw.__wbg_ptr);

        for (const bed_record of record_lines) {
            if (Number.isNaN(bed_record.chromStart)
                || Number.isNaN(bed_record.chromEnd)) {
                continue;
            }

            const record_i = this.records.length;

            const path_name = findPathName(bed_record.chrom);
            const path_range = bedToPathRange(bed_record, path_name);

            const path_cs_raw = await waragraph_viz.worker_obj.pathCoordSys(path_name);
            const path_cs = wasm_bindgen.CoordSys.__wrap(path_cs_raw.__wbg_ptr);
            // const path_step_range = 

            const path_steps = graph.path_steps(path_name);

            const step_range = path_cs.bp_to_step_range(BigInt(path_range.start), BigInt(path_range.end));
            // const step_range = path_cs.bp_to_step_range(path_range.start, path_range.end);
            const path_step_slice = path_steps.slice(step_range.start, step_range.end);

            const record = {
                record_index: record_i,
                bed_record,
                path_name,
                path_range,
                path_step_slice,
            };

            this.records.push(record);
            this.records_viz_state.push({});

        }
    }

    initializeAnnotationPainter() {
        this.annotation_painter =
            new AnnotationPainter(waragraph_viz, this.file_name, this.records);

        let prev_view = waragraph_viz.graph_viewer.getView();

        document.getElementById('viz-svg-overlay')
            .append(this.annotation_painter.svg_root);

        // TODO: there are other cases when this should run, especially once
        // there's more than just 2D-focused SVG
        waragraph_viz
            .graph_viewer
            .view_subject
            .subscribe((view_2d) => {
                const { x, y, width, height } = view_2d;

                let update_pos = false;

                if (prev_view.width !== width
                    || prev_view.height !== height) {
                    update_pos = true;

                    this.annotation_painter.resample2DPaths(view_2d);
                }

                if (prev_view.x != x || prev_view.y !== y || update_pos) {
                    // update SVG path offsets;
                    // should also happen on resample

                    this.annotation_painter.updateSVGPaths(view_2d);
                }

                prev_view = view_2d;
            });

    }

    createListElement() {
        const entries_list = document.createElement('div');
        entries_list.classList.add('bed-file-entry');

        const name_el = document.createElement('div');
        name_el.innerHTML = this.file_name;
        name_el.classList.add('bed-file-name');
        name_el.style.setProperty('flex-basis', '30px');
        entries_list.append(name_el);

        for (const record of this.records) {

            const entry_div = document.createElement('div');
            entry_div.classList.add('bed-file-row');
            // entry_div.style.setProperty('flex-basis', '20px');

            const label_div = document.createElement('div');
            label_div.innerHTML = record.bed_record.name;
            label_div.classList.add('bed-row-label');

            // add checkboxes/buttons for toggling... or just selection?
            // still want something to signal visibility in 1d & 2d, e.g. "eye icons"

            label_div.addEventListener('click', (ev) => {
                ev.stopPropagation();
                let ctx_menu_el = document.getElementById('sidebar-bed-context-menu');
                _context_menu_entry = { record };
                // _context_menu_entry = { bed_entry,
                //                         processed: entry,
                //                         path_name, 
                //                         path_range,
                //                       };

                computePosition(label_div, ctx_menu_el).then(({x, y}) => {
                    ctx_menu_el.style.setProperty('display', 'flex');
                    ctx_menu_el.focus();
                    Object.assign(ctx_menu_el.style, {
                        left: `${x}px`,
                        top: `${y}px`,
                    });
                });
            });

            entry_div.append(label_div);

            entries_list.append(entry_div);
        }

        return entries_list;
    }

    drawOverlayCanvas2d(canvas, view) {
        //
    }

}

async function loadBedFileNew(file) {
    const bed_file = new BEDFile(file.name);
    const bed_text = await file.text();

    const parser = new BED();
    const bed_lines = bed_text.split('\n').map(line => parser.parseLine(line));

    await bed_file.appendRecords(bed_lines);

    bed_file.initializeAnnotationPainter();
    
    const bed_list = document.getElementById('bed-file-list');

    const bed_list_el = bed_file.createListElement();
    bed_list.append(bed_list_el);

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

    const tgl_button = (dim) => {
        const el = document.createElement('button');
        el.innerHTML = dim + 'D';
        el.classList.add('bed-row-' + dim + 'd', 'highlight-disabled');
        el.style.setProperty('display', 'block');
        return el;
    };

    for (const bed_entry of bed_lines) {

        if (!Number.isNaN(bed_entry.chromStart)
            && !Number.isNaN(bed_entry.chromEnd)) {
            const entry = transformBedRange(bed_entry);
            // console.log(entry);

            // const draw_bed = await createDrawBedEntryFn(entry);

            const cb_key = file.name + ":" + entry.name;

            let active = false;

            const entry_div = document.createElement('div');
            entry_div.classList.add('bed-file-row');
            // entry_div.style.setProperty('flex-basis', '20px');

            const label_div = document.createElement('div');
            label_div.innerHTML = entry.name;
            label_div.classList.add('bed-row-label');

            const path_name = findPathName(entry.chrom);
            // const path_range =
            //       await waragraph_viz.worker_obj.pathRangeToStepRange(path_name
            const path_range = await waragraph_viz
                  .worker_obj
                  .pathRangeToStepRange(path_name, entry.chromStart, entry.chromEnd);

            label_div.addEventListener('click', (ev) => {
                ev.stopPropagation();
                let ctx_menu_el = document.getElementById('sidebar-bed-context-menu');
                _context_menu_entry = { bed_entry,
                                        processed: entry,
                                        path_name, 
                                        path_range,
                                      };

                computePosition(label_div, ctx_menu_el).then(({x, y}) => {
                    ctx_menu_el.style.setProperty('display', 'flex');
                    ctx_menu_el.focus();
                    Object.assign(ctx_menu_el.style, {
                        left: `${x}px`,
                        top: `${y}px`,
                    });
                });
            });



            const tgl_1d = tgl_button('1');
            const tgl_2d = tgl_button('2');

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

            const hash_black_color = (record) => {
                if (record.itemRgb === undefined || record.itemRgb === "0,0,0") {
                    let {r,g,b} = wasm_bindgen.path_name_hash_color_obj(record.name);
                    return `rgb(${r * 255},${g * 255},${b * 255})`;
                } else if (typeof record.itemRgb === 'string') {
                    let [r,g,b] = record.itemRgb.split(',');
                    return `rgb(${r * 255},${g * 255},${b * 255})`;
                }
            };

            
            const draw_bed_1d = await createDrawBedEntryFn1d(entry, hash_black_color);
            const el_id = 'viewer-' + path_name;
            console.log(el_id);
            const path_viewer_canvas = document.getElementById('viewer-' + path_name);
            console.log(path_viewer_canvas);
            const path_viewer = document
                  .getElementById('viewer-' + path_name).path_viewer;

            tgl_1d.addEventListener('click', (e) => {
                if (toggle_highlight_class(tgl_1d)) {
                    path_viewer.trackCallbacks[cb_key] = draw_bed_1d;
                } else {
                    delete path_viewer.trackCallbacks[cb_key];
                }
                path_viewer.drawOverlays();
            });

            const draw_bed_2d = await createDrawBedEntryFn2d(entry, hash_black_color);

            tgl_2d.addEventListener('click', (e) => {
                if (toggle_highlight_class(tgl_2d)) {
                    waragraph_viz.graph_viewer.registerOverlayCallback(cb_key, draw_bed_2d);
                } else {
                    waragraph_viz.graph_viewer.removeOverlayCallback(cb_key);
                }
            });

            entries_list.append(entry_div);
        }

    }

    const toggle_div = document.createElement('div');
    toggle_div.classList.add('bed-file-row');
    toggle_div.innerHTML = `
<button class="bed-row-2d">2D</button>
<button class="bed-row-1d">1D</button>`;

    // let tgl_1d = tgl_button('1');
    // let tgl_2d = tgl_button('2');
    // toggle_div.append(tgl_1d);
    // toggle_div.append(tgl_2d);

    toggle_div.querySelector('button.bed-row-2d').addEventListener('click', (e) => {
    // tgl_2d.addEventListener('click', (e) => {
        let buttons = document.querySelectorAll("#bed-file-list > div > div > button.bed-row-2d");
        buttons.forEach((el) => {
            if (e.target !== el) {
                el.click()
            }
        });
    });

    toggle_div.querySelector('button.bed-row-1d').addEventListener('click', (e) => {
    // tgl_1d.querySelector('button.bed-row-1d').addEventListener('click', (e) => {
        let buttons = document.querySelectorAll("#bed-file-list > div > div > button.bed-row-1d");
        // buttons.forEach((el) => el.click());
        buttons.forEach((el) => {
            if (e.target !== el) {
                el.click()
            }
        });
    });

    entries_list.append(toggle_div);


}


async function bedSidebarPanel() {
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

    // const 

    file_button.addEventListener('click', (ev) => {
        for (const file of file_entry.files) {
            // loadBedFile(file);
            loadBedFileNew(file);
        }
    });

    bed_pane.append(file_label);
    bed_pane.append(file_entry);
    bed_pane.append(file_button);

    {

        let graph_raw = await waragraph_viz.worker_obj.getGraph();
        let graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph_raw.__wbg_ptr);

        const context_menu_el = document.createElement('div');
        context_menu_el.id = 'sidebar-bed-context-menu';

        const copy_name_btn = document.createElement('button');

        copy_name_btn.innerHTML = 'Copy name';
        copy_name_btn.addEventListener('click', (ev) => {
            if (_context_menu_entry !== null) {
                let name = _context_menu_entry.bed_record.name;
                if (typeof name === "string") {
                    navigator.clipboard.writeText(name);
                    // context_menu_el.style.setProperty('display', 'none');
                    // _context_menu_entry = null;
                }
            }
        })

        const focus_2d_btn = document.createElement('button');
        focus_2d_btn.innerHTML = 'Focus 2D view';
        focus_2d_btn.addEventListener('click', async (ev) => {
            if (_context_menu_entry === null) {
                return;
            }

            // TODO use something sensible; not just the first step

            let path_name = _context_menu_entry.path_name;
            let path_range = _context_menu_entry.path_range;

            let { start, end } = path_range;

            let path_steps = graph.path_steps(path_name);
            let seg = path_steps[start];

            waragraph_viz.centerViewOnSegment2d(seg);
        });

        context_menu_el.append(copy_name_btn);
        context_menu_el.append(focus_2d_btn);

        document.body.append(context_menu_el);
    }

    document.addEventListener('click', (ev) => {
        let tgt = ev.target;
        let id = "sidebar-bed-context-menu";
        let ctx_menu_el = document.getElementById(id);
        let ctx_open = ctx_menu_el.style.display === 'flex';
        if (!tgt.closest('#' + id) && ctx_open) {
            ctx_menu_el.style.setProperty('display', 'none');
            _context_menu_entry = null;
        }
    });

    return bed_pane;
}


export async function initializeBedSidebarPanel(warapi) {
    waragraph_viz = warapi;

    if (!wasm) {
        wasm = await init_module(undefined, waragraph_viz.wasm.memory);
        wasm_bindgen.set_panic_hook();
    }

    let path_names = await warapi.worker_obj.getPathNames();
    let path_index = 0;

    for (const name of path_names) {
        pathNamesMap.set(name, path_index);
        path_index += 1;
    }

    document
        .getElementById('sidebar')
        .append(await bedSidebarPanel());


}
