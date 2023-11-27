import init_module, * as wasm_bindgen from 'waragraph';

// import BED from '@gmod/bed';

// import {computePosition} from '@floating-ui/dom';

// import { preparePathHighlightOverlay } from '../graph_viewer';

import * as CanvasTracks from './canvas_tracks';


function createSVGElement(tag) {
    return document.createElementNS('http://www.w3.org/2000/svg', tag);
}

let _wasm;

export class AnnotationPainter {
    constructor(waragraph, name, records) {
        this.callback_key = "painter-" + name;

        if (!_wasm) {
            init_module(undefined, waragraph.wasm.memory)
                .then((module) => {
                    _wasm = module;
                    wasm_bindgen.set_panic_hook();
                });
        }


        // this.record_svg_gs = [];

        // this.svg_parent = document.createElementNS('http://www.w3.org/2000/svg', 'svg');

        // this.record_canvas_paths = [];

        this.waragraph = waragraph;

        this.svg_root = createSVGElement('g');
        this.svg_root.id = this.callback_key;

        this.record_states = [];

        this.last_2d_view_scale = null;
        this.last_2d_view_center = null;
        this.last_1d_view = null;

        for (const record of records) {
            const g_el = createSVGElement('g');

            const g_1d = createSVGElement('g');
            g_1d.classList.add('svg-overlay-1d');


            const g_2d = createSVGElement('g');
            g_2d.classList.add('svg-overlay-2d');

            g_1d.setAttribute('display', 'none');
            g_2d.setAttribute('display', 'none');


            g_el.append(g_1d);
            g_el.append(g_2d);

            this.record_states.push({
                svg_g: g_el,
                record,

                enabled: false,

                global_ranges: null,
                cached_path: null,
            });

            this.svg_root.append(g_el);
        }
    }

    async prepareRecords() {


        const cs_view = await this.waragraph.worker_obj.globalCoordSysView();

        for (const state of this.record_states) {
            const { path_name, path_step_slice } = state.record;

            const bed = state.record.bed_record;

            //// set the stroke and fill colors on the root svg element

            let color;

            if (typeof bed.itemRgb === "string") {
                let [r,g,b] = bed.itemRgb.split(',');
                color = `rgb(${r * 255},${g * 255},${b * 255})`;
            } else {
                let {r,g,b} = wasm_bindgen.path_name_hash_color_obj(bed.name);
                color = `rgb(${r * 255},${g * 255},${b * 255})`;
            }

            state.color = color;
            console.warn('color: ', color);

            // state.svg_g.setAttribute('stroke', color);
            state.svg_g.setAttribute('color', color);

            //// global coordinate space rectangles for the 1D path views

            // console.warn(path_step_slice);
            const record_ranges = wasm_bindgen.path_slice_to_global_adj_partitions(path_step_slice);
            // console.warn(record_ranges);

            const ranges_arr = record_ranges.ranges_as_u32_array();
            // console.warn(ranges_arr);
            const range_count = ranges_arr.length / 2;

            const global_ranges = [];

            for (let ri = 0; ri < range_count; ri++) {
                let start_seg = ranges_arr.at(2 * ri);
                let end_seg = ranges_arr.at(2 * ri + 1);
                // this... is probably pretty slow
                // TODO optimize
                if (start_seg !== undefined && end_seg !== undefined) {
                    let start = await cs_view.segmentOffset(start_seg);
                    let end = await cs_view.segmentOffset(end_seg);

                    global_ranges.push({ start, end });
                }
            }

            state.global_ranges = global_ranges;
        }
    }


    resample2DPaths(view_2d_obj) {
        const path_tolerance = 8;

        this.last_2d_view = view_2d_obj;

        // for (let { record, cached_path, enabled } of this.record_states) {
        for (const state of this.record_states) {
            if (!state.enabled) {
                continue;
            }

            const { path_name, path_step_slice, bed_record } = state.record;

            state.cached_path =
                this.waragraph.graph_viewer
                .sampleCanvasSpacePath(path_step_slice, path_tolerance);

            console.warn(state.cached_path);
        }

        // TODO store view... scale? & use to decide when to resample
        // probably do that in caller
    }

    updateSVG1D(view_1d) {

        const svg_rect = document.getElementById('viz-svg-overlay').getBoundingClientRect();

        const view_len = view_1d.end - view_1d.start;

        for (const { svg_g, record, global_ranges, enabled, color } of this.record_states) {
            if (global_ranges === null) {
                continue;
            }

            const svg_g_1d = svg_g.querySelector('.svg-overlay-1d');

            // console.warn(`updating for record ${record}`);
            // console.warn(svg_g_1d);

            const data_canvas = document.getElementById('viewer-' + record.path_name);
            const data_rect = data_canvas.getBoundingClientRect();

            svg_g_1d.innerHTML = "";
            for (const { start, end } of global_ranges) {
                const el_rect = createSVGElement('rect');

                svg_g_1d.append(el_rect);

                // map global range to `data_rect` via `view_1d`

                let r_start = (start - view_1d.start) / view_len;
                let r_end = (end - view_1d.start) / view_len;

                let screen_rs_x = data_rect.left + r_start * data_rect.width;

                let y = 100 * (data_rect.top - svg_rect.top) / svg_rect.height;
                let x = 100 * (screen_rs_x - svg_rect.left) / svg_rect.width;

                let width = 100 * (r_end - r_start) * data_rect.width;
                let height = 100 * data_rect.height / svg_rect.height;

                el_rect.outerHTML = `<rect x="${x}" y="${y}" width="${width}" height="${height}"
fill="${color}"
/>`;
            }

        }

    }

    updateSVGPaths(view_2d) {
        const canvas = document.getElementById("graph-viewer-2d");
        const w = canvas.width;
        const h = canvas.height;

        // const canvas_rect = canvas.getBoundingClientRect();

        const svg_rect =
              document.getElementById('viz-svg-overlay')
              .getBoundingClientRect();


        const height_prop = canvas.height / svg_rect.height;
        const map_canvas_to_svg = ( {x, y} ) => {
            let x_ = 100 * x / canvas.width;
            let y_ = 100 * height_prop * y / canvas.height;
            return { x: x_, y: y_ };
        };

        for (const { svg_g, record, cached_path, enabled, color } of this.record_states) {

            if (!enabled || cached_path === null) {
                // svg_g.innerHTML = '';
                // svg_g.style.setProperty('display', 'none');
                continue;
            }

            let svg_path = "";

            cached_path.with_points((x, y) => {
                const p = map_canvas_to_svg({x, y});
                
                if (svg_path.length === 0) {
                    svg_path += `M ${p.x},${p.y}`;
                } else {
                    svg_path += ` L ${p.x},${p.y}`;
                }
            });

            svg_g.querySelector('.svg-overlay-2d').innerHTML =
            // svg_g.innerHTML =
                // `<path d="${svg_path}" stroke-width="0.5" fill="none" />`;
            `<path d="${svg_path}" stroke-width="0.5" stroke="${color}" fill="none" />`;
                // `<path d="${svg_path}" stroke-width="0.5" stroke="red" fill="none" />`;
        }

    }

    /*
    // drawing to the canvas/updating the SVG based on view offset is
    // different to resampling the path for the current 2D view scale
    async update(view_1d, view_2d) {

        for (const { cached_path, g, enabled } of this.record_states) {
            if (enabled !== true) {
                // set `g` to display none?
                continue;
            }

            // update cached path



        }



    }
    */


}

