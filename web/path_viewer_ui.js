
async function addOverviewEventHandlers(path_viewer, overview) {
    overview.canvas.addEventListener("wheel", (event) => {
        console.log("scrolling");
        console.log(event);

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

        console.log(mode);
        console.log(delta);


    });

    const view_max = await path_viewer.maxView();

    const centerAround = (mx) => {
        let bp_pos = (mx / overview.canvas.width) * view_max;
        path_viewer.centerViewAt(bp_pos);
    };

    overview.canvas.addEventListener("mousedown", (event) => {
        centerAround(event.clientX);
    });

    overview.canvas.addEventListener("mousemove", (event) => {
        if (event.buttons == 1) {
            centerAround(event.clientX);
        }
    });
}

export async function addPathViewerEventHandlers(worker, path_viewer, canvas, overview) {
    console.log("adding path viewer event handlers & glue");

    const coord_sys = await path_viewer.coord_sys;
    console.log("coord_sys");
    console.log(coord_sys);


    // const { fromEvent,
    //         takeUntil,
    //         takeWhile, 
    //         finalize,
    //         mergeMap,
    //         map,
            

    const state = {
        dragging: false,
        dragOrigin: null,
    };

    const startDrag = (ev) => {
        state.dragging = true;
        state.dragOrigin = ev.clientX;
    };

    const stopDrag = (ev) => {
        state.dragging = false;
        state.dragOrigin = null;
    };

    await addOverviewEventHandlers(path_viewer, overview);


    const { fromEvent,
            exhaustMap,
            map,
            mergeMap,
            takeUntil,
            scan,
            tap
          } = rxjs;

    rxjs.fromEvent(canvas, 'mousedown')
        .pipe(
            mergeMap(_ => fromEvent(canvas, 'mousemove')),
            map((event) => event.clientX),
            // tap((x) => {
            //     console.log("x: " + x);
            // })
            takeUntil(
                rxjs.merge(
                    fromEvent(canvas, 'mouseup'),
                    fromEvent(canvas, 'mouseout'),
                )
            ),
            scan((last, current) => current - last),
            // map(async (delta_x) => {
            //     let view = await path_viewer.getView();
            //     console.log("view: " + view);
            //     return { delta_x, view };
            // }),
            // tap(async ({ delta_x, view }) => {
            tap(async (delta_x) => {
                let view = await path_viewer.getView();
                console.log("view????? " + view);
                let { left, right } = view;
                let view_size = (right - left + 1);
                let delta_bp = (delta_x / canvas.width) * view_size;
                path_viewer.translateView(delta_bp);
            }),
            rxjs.finalize(() => {
                console.log("done+??=?`????????");
            }),

            // map((
        )
        .subscribe();
                   


    /*
    canvas.addEventListener("mousedown", startDrag);
    canvas.addEventListener("mouseout", stopDrag);
    canvas.addEventListener("mouseup", stopDrag);

    canvas.addEventListener("mousemove", (event) => {
        let mx = event.clientX;

        path_viewer.getView().then((view) => {
            let { left, right } = view;
            let view_size = (right - left + 1);

            let bp_pos = left + (mx / canvas.width) * view_size;

            if (state.dragging === true) {
                let drag_delta = (state.dragOrigin - mx) / canvas.width;
                let del_bp = drag_delta * view_size;

                path_viewer.translateView(del_bp);
                state.dragOrigin = mx;
            }

        });
    });
    */

    let last_view = null;
    const interval_id = setInterval(() => {
        path_viewer.getView().then((cur_view) => {

            let need_refresh;

            if (last_view === null) {
                // console.log("last view null");
                need_refresh = true;
            } else {
                let views_equal = last_view.left == cur_view.left
                    && last_view.right == cur_view.right;
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
