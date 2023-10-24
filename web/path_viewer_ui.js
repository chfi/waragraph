

// async function addScrollZoomSub(cs_view
import * as Comlink from './comlink.mjs';

import * as handler from './transfer_handlers.js';
handler.setTransferHandlers(rxjs, Comlink);


export async function highlightPathRanges(
    path_name,
    pixel_ranges,
    color
) {
    let overlay = document.getElementById("overlay-" + path_name);

    if (overlay === undefined) {
        return;
    }

    let ctx = overlay.getContext('2d');

    ctx.save();

    for (const { start, end } of pixel_ranges) {
        ctx.fillRect(start, 0, end - start, overlay.height);
    }

    ctx.restore();
}


// creates and attaches a path viewer to a canvas
// does not attach the canvas element to the DOM
export async function initializePathViewer(
    worker,
    overview,
    cs_view,
    path_name,
    canvas
) {
    if (canvas === undefined) {
        canvas = document.createElement('canvas');
    }

    canvas.width = 1024;
    canvas.height = 40;

    let offscreen = canvas.transferControlToOffscreen();

    const worker_ctx = await worker.createPathViewer(Comlink.transfer(offscreen, [offscreen]),
                                                      path_name);

    let view = await cs_view.get();

    worker_ctx.setView(view.start, view.end);
    worker_ctx.sample();
    worker_ctx.forceRedraw();

    const trackCallbacks = {};

    return { worker_ctx, canvas, trackCallbacks };
}


export async function addPathViewerLogic(worker, path_viewer, overview, cs_view) {
    const { worker_ctx, overlay_canvas } = path_viewer;
    const canvas = overlay_canvas;

    const { fromEvent,
            map,
            pairwise,
            race,
            switchMap,
            takeUntil,
          } = rxjs;

    const wheel$ = rxjs.fromEvent(canvas, 'wheel');
    const mouseDown$ = fromEvent(canvas, 'mousedown');
    const mouseUp$ = fromEvent(canvas, 'mouseup');
    const mouseMove$ = fromEvent(canvas, 'mousemove');
    const mouseOut$ = fromEvent(canvas, 'mouseout');

    const wheelScaleDelta$ = wheel$.pipe(
        map(event => {
            let x = event.clientX / canvas.width;
            // if (x < 0.03) {
            //     x = 0.0;
            // } else if (x > 0.97) {
            //     x = 1.0;
            // }
            if (event.deltaY > 0) {
                return { scale: 1.05, x };
            } else {
                return { scale: 0.95, x };
            }
        })
    );

    await cs_view.subscribeZoomAround(wheelScaleDelta$);

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

    const dragDeltaNorm$ = drag$.pipe(rxjs.map((delta_x) => {
        let delta = (delta_x / canvas.width);
        return -delta;
    }));

    await cs_view.subscribeTranslateDeltaNorm(dragDeltaNorm$);

    let view_subject = await cs_view.viewSubject();

    worker_ctx.sample();
    worker_ctx.forceRedraw();

    view_subject.pipe(
        rxjs.distinct(),
        rxjs.throttleTime(10)
    ).subscribe((view) => {
        requestAnimationFrame((time) => {
            worker_ctx.setView(view.start, view.end);
            worker_ctx.sample();
            worker_ctx.forceRedraw();

            let overview_ctx = canvas.getContext('2d');
            overview_ctx.clearRect(0, 0, canvas.width, canvas.height);

            for (const key in path_viewer.trackCallbacks) {
                const callback = path_viewer.trackCallbacks[key];
                callback(canvas, view);
            }

        });
    });

}

export async function addOverviewEventHandlers(overview, cs_view) {

    const { fromEvent,
            map,
            pairwise,
            race,
            switchMap,
            takeUntil,
          } = rxjs;

    const wheel$ = rxjs.fromEvent(overview.canvas, 'wheel');
    const mouseDown$ = rxjs.fromEvent(overview.canvas, 'mousedown');
    const mouseUp$ = rxjs.fromEvent(overview.canvas, 'mouseup');
    const mouseMove$ = rxjs.fromEvent(overview.canvas, 'mousemove');
    const mouseOut$ = rxjs.fromEvent(overview.canvas, 'mouseout');

    const view_max = await cs_view.viewMax();


    const wheelScaleDelta$ = wheel$.pipe(
        map(event => {
            if (event.deltaY > 0) {
                return 1.05;
            } else {
                return 0.95;
            }
        })
    );

    await cs_view.subscribeZoomCentered(wheelScaleDelta$);

    const mouseAt$ = mouseDown$.pipe(
        switchMap((event) => {
            return mouseMove$.pipe(
                map((ev) => (ev.clientX / overview.canvas.width) * view_max),
                takeUntil(
                    race(mouseUp$, mouseOut$)
                )
            )
        })
    );

    await cs_view.subscribeCenterAt(mouseAt$);


    let view_subject = await cs_view.viewSubject();

    view_subject.pipe(
        rxjs.distinct(),
        rxjs.throttleTime(10),
    ).subscribe((view) => {
        requestAnimationFrame(() => {
            overview.draw(view);
        })
    });

}
