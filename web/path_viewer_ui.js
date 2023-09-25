

// async function addScrollZoomSub(cs_view
import * as Comlink from "https://unpkg.com/comlink/dist/esm/comlink.mjs";


async function addScrollZoomHandler(path_viewer, element) {
    console.log(element);
    element.addEventListener("wheel", (event) => {
        // console.log("scrolling");
        // console.log(event);

        let mode = event.deltaMode;
        let delta = event.deltaY;

        let relative_scale;

        if (mode === WheelEvent.DOM_DELTA_PIXEL) {
            //
        } else if (mode === WheelEvent.DOM_DELTA_PAGE) {
            //
        }

        if (delta > 0) {
            path_viewer.zoomViewCentered(1.05);
        } else if (delta < 0) {
            path_viewer.zoomViewCentered(0.95);
        }

        // console.log(mode);
        // console.log(delta);
    });
}

async function addOverviewEventHandlers(path_viewer, overview) {
    addScrollZoomHandler(path_viewer, overview.canvas);

    const wheel$ = rxjs.fromEvent(overview.canvas, 'wheel');
    const mouseDown$ = rxjs.fromEvent(overview.canvas, 'mousedown');
    const mouseUp$ = rxjs.fromEvent(overview.canvas, 'mouseup');
    const mouseMove$ = rxjs.fromEvent(overview.canvas, 'mousemove');
    const mouseOut$ = rxjs.fromEvent(overview.canvas, 'mouseout');

    // wheel$.pipe(

    // );


    const view_max = await path_viewer.maxView();

    const centerAround = (mx) => {
        let bp_pos = (mx / overview.canvas.width) * view_max;
        path_viewer.centerViewAt(bp_pos);
    };

    // mouseDown$
    //     .pipe(

    overview.canvas.addEventListener("mousedown", (event) => {
        centerAround(event.clientX);
    });

    overview.canvas.addEventListener("mousemove", (event) => {
        if (event.buttons == 1) {
            centerAround(event.clientX);
        }
    });
}

export async function addPathViewerLogic(worker, path_viewer, canvas, overview, cs_view) {
    const { fromEvent,
            map,
            pairwise,
            race,
            switchMap,
            takeUntil,
          } = rxjs;

    const mouseDown$ = fromEvent(canvas, 'mousedown');
    const mouseUp$ = fromEvent(canvas, 'mouseup');
    const mouseMove$ = fromEvent(canvas, 'mousemove');
    const mouseOut$ = fromEvent(canvas, 'mouseout');


    const drag$ = mouseDown$.pipe(
        switchMap((event) => {
            return mouseMove$.pipe(
                pairwise(),
                map(([prev, current]) => current.clientX - prev.clientX),
                takeUntil(
                    race(mouseUp$, mouseOut$)
                )
            )
        })
    );

    // const dragDeltaNorm$ = drag$.pipe(rxjs.map(async (delta_x) => {
    const dragDeltaNorm$ = drag$.pipe(rxjs.map((delta_x) => {
        let delta = (delta_x / canvas.width);
        return -delta;
    }));


    console.log(dragDeltaNorm$);

    console.log("-----------------------------");
    console.log(cs_view);
    await cs_view.subscribeTranslateDeltaNorm(dragDeltaNorm$);




    // cs_view.subscribeTest(15);
    // cs_view.subscribeTest(Comlink.proxy((wtf) => {
    //     console.log("in callback!");
    // }));
    // cs_view.subscribeTranslateDeltaNorm(Comlink.proxy(dragDeltaNorm$));

    /*
    // await cs_view.subscribeTranslateDeltaNorm(Comlink.proxy(dragDeltaNorm$));
    console.log("oaky");
    */

    // drag$.subscribe(async (delta_x) => {
    //     let { start, end, len } = await path_viewer.getView();
    //     let delta_bp = (delta_x / canvas.width) * len;
    //     console.log("in subscribe?!?");
    //     // path_viewer.translateView(-delta_bp);
    // });

}

export async function addPathViewerEventHandlers(worker, path_viewer, canvas, overview) {
    console.log("adding path viewer event handlers & glue");

    const coord_sys = await path_viewer.coord_sys;
    console.log("coord_sys");
    console.log(coord_sys);

    await addOverviewEventHandlers(path_viewer, overview);


    const { fromEvent,
            map,
            pairwise,
            race,
            switchMap,
            takeUntil,
          } = rxjs;

    const mouseDown$ = fromEvent(canvas, 'mousedown');
    const mouseUp$ = fromEvent(canvas, 'mouseup');
    const mouseMove$ = fromEvent(canvas, 'mousemove');
    const mouseOut$ = fromEvent(canvas, 'mouseout');


    const drag$ = mouseDown$.pipe(
        switchMap((event) => {
            return mouseMove$.pipe(
                pairwise(),
                map(([prev, current]) => current.clientX - prev.clientX),
                takeUntil(
                    race(mouseUp$, mouseOut$)
                )
            )
        })
    );

    /*
    drag$.subscribe(async (delta_x) => {
        let { start, end, len } = await path_viewer.getView();
        let delta_bp = (delta_x / canvas.width) * len;
        path_viewer.translateView(-delta_bp);
    });
    */

    addScrollZoomHandler(path_viewer, canvas);

    let last_view = null;
    const interval_id = setInterval(() => {
        path_viewer.getView().then((cur_view) => {

            let need_refresh;

            if (last_view === null) {
                // console.log("last view null");
                need_refresh = true;
            } else {
                let views_equal = last_view.start == cur_view.start
                    && last_view.end == cur_view.end;
                // console.log("views equal: " + views_equal);

                // console.log(last_view);
                // console.log(cur_view);

                need_refresh = !views_equal;
            };

            if (need_refresh) {
                // console.log("left: " + cur_view.left + ", right: " + cur_view.right);
                requestAnimationFrame((time) => {
                    path_viewer.sample();
                    path_viewer.forceRedraw();
                    overview.draw(cur_view);
                });
                last_view = cur_view;
            }

        });

    }, 50);
        
}
