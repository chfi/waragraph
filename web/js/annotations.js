import init_module, * as wasm_bindgen from '../pkg/web.js';

// import BED from '@gmod/bed';

// import {computePosition} from '@floating-ui/dom';

// import { preparePathHighlightOverlay } from '../graph_viewer.js';

import * as CanvasTracks from '../canvas_tracks.js';


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

            this.record_states.push({
                svg_g: g_el,
                record,
                cached_path: null,
                enabled: true,
            });

            this.svg_root.append(g_el);
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

        for (const { svg_g, record, cached_path, enabled } of this.record_states) {
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


            svg_g.innerHTML =
                `<path d="${svg_path}" stroke-width="0.5" stroke="red" fill="none" />`;
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

