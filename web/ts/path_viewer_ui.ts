import * as wasm_bindgen from 'waragraph';

import type { Bp, Segment, Handle, PathId, RGBObj } from './types';
import { type WithPtr, wrapWasmPtr } from './wrap';

import { type Viewport1D } from './waragraph';
import { type WaragraphWorkerCtx, type PathViewerCtx } from './new_worker';

import { placeTooltipAtPoint } from './tooltip';

import * as Comlink from 'comlink';

import { fromEvent, map, pairwise, race, switchMap, takeUntil, Observable } from 'rxjs';
import * as rxjs from 'rxjs';


import * as handler from './transfer_handlers';
handler.setTransferHandlers(rxjs, Comlink);

import * as FloatingUI from '@floating-ui/dom';

async function segmentAtCanvasX(
  // coord_sys_view,
  viewport: Viewport1D,
  canvas_width: number,
  x: number
) {
  let { start, end } = viewport.get();
  let len = end - start;

  let bp_f = start + (x / canvas_width) * len;
  let bp = BigInt(Math.round(bp_f));

  let segment = viewport.segmentAtOffset(bp);

  return segment;
}

export async function highlightPathRanges(
  path_name,
  pixel_ranges,
  color
) {
  let overlay = document.getElementById("overlay-" + path_name) as HTMLCanvasElement;

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


export interface PathViewer {
  drawOverlays: () => Promise<void>;
  onResize: () => Promise<void>;

  path_name: string;
  viewport: Viewport1D;

  // worker_ctx: Comlink.Remote<any>;
  worker_ctx: Comlink.Remote<PathViewerCtx & Comlink.ProxyMarked>;

  data_canvas: HTMLCanvasElement & { path_viewer: PathViewer };
  overlay_canvas: HTMLCanvasElement;
  container: HTMLDivElement;

  trackCallbacks: Object;
}

export interface WithPathViewer {
  path_viewer: PathViewer;
}


export async function initializePathViewer(
  worker_ctx: Comlink.Remote<WaragraphWorkerCtx>,
  path_name: string,
  viewport: Viewport1D,
  // coord_sys: wasm_bindgen.CoordSys & WithPtr,
  data: wasm_bindgen.SparseData,
  threshold: number,
  color_below: RGBObj,
  color_above: RGBObj,
) {

  const container = document.createElement('div');

  const data_canvas = document.createElement('canvas');

  let width = container.clientWidth;
  let height = container.clientHeight;

  data_canvas.width = width;
  // data_canvas.height = height;
  data_canvas.height = 20;



  data_canvas.id = 'viewer-' + path_name;

  let offscreen = data_canvas.transferControlToOffscreen();

  const viewer_ctx =
    await worker_ctx.createPathViewer(
      Comlink.transfer(offscreen, [offscreen]),
      viewport.coord_sys as wasm_bindgen.CoordSys & WithPtr,
      path_name,
      data,
      threshold,
      color_below,
      color_above
    );

  const overlay_canvas = document.createElement('canvas');
  overlay_canvas.width = width;
  overlay_canvas.height = height;

  data_canvas.style.setProperty('z-index', '0');
  overlay_canvas.style.setProperty('z-index', '1');

  data_canvas.classList.add('path-data-canvas');
  overlay_canvas.classList.add('path-data-canvas');

  await viewer_ctx.setCanvasWidth(width);

  container.append(data_canvas);
  container.append(overlay_canvas);

  {
    let view = viewport.get();

    await viewer_ctx.setView(view.start, view.end);
    await viewer_ctx.sample();
    await viewer_ctx.forceRedraw();
  }


  /*
  resize_subject.subscribe(async () => {
    let w = container.clientWidth;
    let h = container.clientHeight;

    console.log(container);

    await viewer_ctx.setCanvasWidth(w);
    await viewer_ctx.resizeTargetCanvas(w, h);
    await viewer_ctx.sample();
    await viewer_ctx.forceRedraw();

    overlay_canvas.width = w;
    overlay_canvas.height = h;
  });
    */
  const trackCallbacks = {};

  const path_viewer = {
    worker_ctx: viewer_ctx,
    data_canvas,
    overlay_canvas,
    container,
    trackCallbacks,
    viewport,
    path_name
  } as PathViewer;

  path_viewer.onResize = async () => {
    let w = container.clientWidth;
    let h = container.clientHeight;

    console.warn("resizing path viewer");
    console.warn(container);

    console.warn("width: ", w, ", height: ", h);

    await viewer_ctx.setCanvasWidth(w);
    // await viewer_ctx.setCanvasDims({ width: w, height: h });
    // await viewer_ctx.setCanvasDims(w, h);
    await viewer_ctx.resizeTargetCanvas(w, h);
    await viewer_ctx.sample();
    await viewer_ctx.forceRedraw();

    overlay_canvas.width = w;
    overlay_canvas.height = h;
  };

  path_viewer.drawOverlays = async () => {
    let canvas = overlay_canvas;
    let overlay_ctx = canvas.getContext('2d');
    overlay_ctx?.clearRect(0, 0, canvas.width, canvas.height);

    let view = viewport.get();

    for (const key in path_viewer.trackCallbacks) {
      const callback = path_viewer.trackCallbacks[key];
      callback(canvas, view);
    }
  };

  path_viewer.data_canvas.path_viewer = path_viewer;
  // data_canvas.path_viewer = path_viewer;

  return path_viewer;
}


export async function addPathViewerLogic(
  worker: Comlink.Remote<WaragraphWorkerCtx>,
  path_viewer: PathViewer,
) {
  const { worker_ctx, overlay_canvas } = path_viewer;
  const canvas = overlay_canvas;

  const wheel$ = fromEvent(canvas, 'wheel').pipe(
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
    rxjs.distinct(),
    rxjs.throttleTime(50)
  ).subscribe(async (e: MouseEvent) => {
    let x = e.offsetX;
    let y = e.offsetY;
    let width = canvas.width;

    let segment = await segmentAtCanvasX(path_viewer.viewport, width, x);
    // console.log("segment at cursor: " + segment);

    let tooltip = document.getElementById('tooltip');

    tooltip.innerHTML = `Segment ${segment}`;
    tooltip.style.display = 'block';
    // console.warn("x: ", e.clientX, ", y: ", e.clientY);
    placeTooltipAtPoint(e.clientX, e.clientY);
    // placeTooltipAtPoint(x, y);

    let paths = await worker.pathsOnSegment(segment);
    // console.log("paths!!!");
    // console.log(paths);
  });


  const wheelScaleDelta$ = wheel$.pipe(
    map((event: WheelEvent) => {
      let x = event.offsetX / canvas.width;
      let scale = 1.0;
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

  wheelScaleDelta$.subscribe(({ scale, x }) => {
    path_viewer.viewport.zoomNorm(x, scale);
  });

  const drag$ = mouseDown$.pipe(
    switchMap((event: MouseEvent) => {
      return mouseMove$.pipe(
        // pairwise(),
        map((ev: MouseEvent) => ev.movementX),
        takeUntil(
          race(mouseUp$, mouseOut$)
        )
      )
    })
  );

  // const drag$ = mouseDown$.pipe(
  //   rxjs.switchMap((event: MouseEvent) => {
  //     return mouseMove$.pipe(
  //       rxjs.takeUntil(
  //         rxjs.race(mouseUp$, mouseOut$)
  //       )
  //     )
  //   })
  // );

  // const dragDeltaNorm$ = drag$.pipe(rxjs.map((ev: MouseEvent) => {
  const dragDeltaNorm$ = drag$.pipe(rxjs.map((dx: number) => {
    let delta = (dx / canvas.width);
    return -delta;
  }));

  dragDeltaNorm$.subscribe((delta: number) => {
      let delta_bp = delta * path_viewer.viewport.length;
      path_viewer.viewport.translateView(delta_bp);
  });

  let view_subject = path_viewer.viewport.subject;

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
      overlay_ctx?.clearRect(0, 0, canvas.width, canvas.height);

      for (const key in path_viewer.trackCallbacks) {
        const callback = path_viewer.trackCallbacks[key];
        callback(canvas, view);
      }

    });
  });

}

export async function addOverviewEventHandlers(overview, viewport: Viewport1D) {


  const wheel$ = rxjs.fromEvent<WheelEvent>(overview.canvas, 'wheel');
  const mouseDown$ = rxjs.fromEvent<MouseEvent>(overview.canvas, 'mousedown');
  const mouseUp$ = rxjs.fromEvent<MouseEvent>(overview.canvas, 'mouseup');
  const mouseMove$ = rxjs.fromEvent<MouseEvent>(overview.canvas, 'mousemove');
  const mouseOut$ = rxjs.fromEvent<MouseEvent>(overview.canvas, 'mouseout');

  const view_max = viewport.max;


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

  wheelScaleDelta$.subscribe((scale: number) => {
    viewport.zoomViewCentered(scale);
  });

  const mouseAt$ = mouseDown$.pipe(
    switchMap((event: MouseEvent) => {
      return mouseMove$.pipe(
        map((ev) => (ev.offsetX / overview.canvas.width) * view_max),
        takeUntil(
          race(mouseUp$, mouseOut$)
        )
      )
    })
  );

  // await cs_view.subscribeCenterAt(mouseAt$);

  mouseAt$.subscribe((bp_pos) => {
    viewport.centerAt(bp_pos);
  });


  let view_subject = viewport.subject;

  view_subject.pipe(
    rxjs.distinct(),
    rxjs.throttleTime(10),
  ).subscribe((view) => {
    requestAnimationFrame(() => {
      overview.draw(view);
    })
  });

}
