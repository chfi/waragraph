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

        let layout_tsv = await fetch("./data/A-3105.layout.tsv").then(l => l.text());

        console.log("main - initializing 2D viewer");
        console.log(layout_tsv);
        let viewer_2d = worker_obj.initialize2DGraphViewer(layout_tsv);

        let names = await worker_obj.getPathNames();
        console.log(names);

        let cs_view = await worker_obj.globalCoordSys();

        let view_max = await cs_view.viewMax();

        let overview_el = document.getElementById('overview-map');
        // console.log(overview_el);
        // overview_el.width = overview_el.parentElement.clientWidth;
        let overview = new OverviewMap(overview_el,  view_max);
        await addOverviewEventHandlers(overview, cs_view);

        // let path_viewer = await initializePathViewer(worker_obj, overview, cs_view, path_name, canvas);

        let container = document.getElementById('container');
        // let name_container = container.querySelector('.path-name-column');
        // let data_container = container.querySelector('.path-data-view-column');
        // console.log(name_container);
        // console.log(data_container);

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

            container.append(row_el);

            console.log(canvas);
            console.log(canvas.clientWidth);

            if (path_ix == 0) {
                overview_el.width = canvas.clientWidth;
            }

            // let parent_w = data_container.clientWidth;
            // console.log(parent_w);
            // await path_viewer.setCanvasWidth(parent_w);

        });

// div.path-viewer-list-row:nth-child(2) > div:nth-child(1)

        // let col2 = document.querySelector("canvas.path-data-view");
        // console.log(col2);


        window.worker_obj = worker_obj;
        
    };

}

init();
