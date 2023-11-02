import * as Comlink from './comlink.mjs';

import * as handler from './transfer_handlers.js';
handler.setTransferHandlers(rxjs, Comlink);

// import * as FloatingUI from 'https://cdn.jsdelivr.net/npm/@floating-ui/dom@1.5.3/+esm';
import * as FloatingUI from '@floating-ui/dom';

import { placeTooltipAtPoint } from './tooltip.js';

async function segmentAtCanvasX(
    coord_sys_view,
    canvas_width,
    x
) {
    let { start, end, len } = await coord_sys_view.get();

    let bp_f = start + (x / canvas_width) * len;
    let bp = BigInt(Math.round(bp_f));

    let segment = await coord_sys_view.segmentAtOffset(bp);

    return segment;
}

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

    const wheel$ = rxjs.fromEvent(canvas, 'wheel').pipe(
        rxjs.tap(event => event.preventDefault())
    );
    const mouseDown$ = fromEvent(canvas, 'mousedown');
    const mouseUp$ = fromEvent(canvas, 'mouseup');
    const mouseMove$ = fromEvent(canvas, 'mousemove');
    const mouseOut$ = fromEvent(canvas, 'mouseout');


    mouseOut$.subscribe((ev) => {
        let tooltip = document.getElementById('tooltip');
        tooltip.innerHTML = "";
        tooltip.style.display = 'none';
    });
        
    
    mouseMove$.pipe(
        map((e) => [e.clientX, e.clientY]),
        rxjs.distinct(),
        rxjs.throttleTime(50)
    ).subscribe(async ([x, y]) => {

        let local_x = x - canvas.offsetLeft;
        let width = canvas.width;
        let segment = await segmentAtCanvasX(cs_view, width, local_x);
        console.log("segment at cursor: " + segment);

        let tooltip = document.getElementById('tooltip');

        tooltip.innerHTML = `Segment ${segment}`;
        tooltip.style.display = 'block';
        placeTooltipAtPoint(x, y);
    });


    const wheelScaleDelta$ = wheel$.pipe(
        map(event => {
            let x = (event.clientX - canvas.offsetLeft) / canvas.width;
            let scale;
            if (event.deltaMode === WheelEvent.DOM_DELTA_PIXEL) {
                if (event.deltaY > 0) {
                    scale = 1.01;
                } else {
                    scale = 0.99;
                }
            // } else if (event.deltaMode == WheelEvent.DOM_DELTA_LINE) {
            } else {
                if (event.deltaY > 0) {
                    scale = 1.05;
                } else {
                    scale = 0.95;
                }
            }

            return { scale, x };
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

            let overlay_ctx = canvas.getContext('2d');
            overlay_ctx.clearRect(0, 0, canvas.width, canvas.height);

            for (const key in path_viewer.trackCallbacks) {
                const callback = path_viewer.trackCallbacks[key];
                // console.log(callback);
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
            if (event.deltaMode === WheelEvent.DOM_DELTA_PIXEL) {
                if (event.deltaY > 0) {
                    return 1.01;
                } else {
                    return 0.99;
                }
            // } else if (event.deltaMode == WheelEvent.DOM_DELTA_LINE) {
            } else {
                if (event.deltaY > 0) {
                    return 1.05;
                } else {
                    return 0.95;
                }
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
