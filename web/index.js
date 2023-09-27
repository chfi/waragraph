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


        let canvas = document.getElementById('path_view');
        let path_name = "gi|528476637:29857558-29915771";

        let cs_view = await worker_obj.globalCoordSys();

        let view_max = await cs_view.viewMax();

        let overview = new OverviewMap(document.getElementById('overview_map'),  view_max);
        await addOverviewEventHandlers(overview, cs_view);

        // let path_viewer = await initializePathViewer(worker_obj, overview, cs_view, path_name, canvas);

        names.forEach(async (name, path_ix) => {

            let { path_viewer, canvas } =
                await initializePathViewer(worker_obj,
                                           overview,
                                           cs_view,
                                           path_name);

            // canvas.width = 800;
            // canvas.height = 40;
            // document.append(canvas);
        });

        window.worker_obj = worker_obj;
        
    };

}

init();
