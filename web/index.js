import * as Comlink from './comlink.mjs';

const { Observable } = rxjs;

import { initializePathViewer, addOverviewEventHandlers, addPathViewerLogic } from './path_viewer_ui.js';

import init_module, * as wasm_bindgen from './pkg/web.js';

import { OverviewMap } from './overview.js';

async function init() {

    const handler = await import('./transfer_handlers.js');
    handler.setTransferHandlers(rxjs, Comlink);


    const worker = new Worker("main_worker.js", { type: 'module' });

    worker.onmessage = async (event) => {
        worker.onmessage = undefined;

        console.log("graph loaded");
        const worker_obj = Comlink.wrap(worker);

        let names = await worker_obj.getPathNames();
        console.log(names);

        let cs_view = await worker_obj.globalCoordSys();

        let view_max = await cs_view.viewMax();

        let overview = new OverviewMap(document.getElementById('overview_map'),  view_max);
        await addOverviewEventHandlers(overview, cs_view);

        // let path_viewer = await initializePathViewer(worker_obj, overview, cs_view, path_name, canvas);

        let container = document.getElementById('container');

        names.forEach(async (name, path_ix) => {

            console.log("path: " + name);

            let { path_viewer, canvas } =
                await initializePathViewer(worker_obj,
                                           overview,
                                           cs_view,
                                           name);

            const row = document.createElement("div");
            row.classList.add("path-row");


            const name_el = document.createElement("div");
            name_el.classList.add("path-name");
            name_el.innerHTML = name;

            let id = "viewer-" + name;
            console.log(canvas);

            // const canvas_col = document.createElement("div");
            // canvas_col.classList.add("path-data-view");

            canvas.classList.add("path-data-view");
            // canvas_col.append(canvas);

            // container.append(canvas);
            canvas.id = id;
            container.append(name_el);
            // row.append(canvas);
            // row.append(canvas_col);
            // canvas_col.append(canvas);
            container.append(canvas);

        });

        window.worker_obj = worker_obj;
        
    };

}

init();
