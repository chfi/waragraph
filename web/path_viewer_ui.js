

// async function addScrollZoomSub(cs_view
import * as Comlink from './comlink.mjs';

import * as handler from './transfer_handlers.js';
handler.setTransferHandlers(rxjs, Comlink);



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

    let offscreen = canvas.transferControlToOffscreen();

    const path_viewer = await worker.createPathViewer(Comlink.transfer(offscreen, [offscreen]),
                                                      path_name);

    addPathViewerLogic(worker, path_viewer, canvas, overview, cs_view);

    let view = await cs_view.get();

    path_viewer.setView(view.start, view.end);
    path_viewer.sample();
    path_viewer.forceRedraw();
}


export async function addPathViewerLogic(worker, path_viewer, canvas, overview, cs_view) {

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
            if (event.deltaY > 0) {
                return 1.05;
            } else {
                return 0.95;
            }
        })
    );

    await cs_view.subscribeZoomCentered(wheelScaleDelta$);

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
    console.log(view_subject);

    path_viewer.sample();
    path_viewer.forceRedraw();

    view_subject.pipe(
        rxjs.distinct(),
        rxjs.throttleTime(10)
    ).subscribe((view) => {
        requestAnimationFrame((time) => {
            path_viewer.setView(view.start, view.end);
            path_viewer.sample();
            path_viewer.forceRedraw();
        });
    });

    /*
    let is_busy = false;

    view_subject.subscribe((view) => {
        if (!is_busy) {
            is_busy = true;
            // adding a hacky delay to remember to fix this later
            setTimeout(() => {
                requestAnimationFrame((time) => {
                    path_viewer.setView(view.start, view.end);
                    path_viewer.sample();
                    path_viewer.forceRedraw();
                    // overview.draw(view);
                    is_busy = false;
                });
            }, 10);
        }
    });
    */

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
