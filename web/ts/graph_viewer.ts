import { BehaviorSubject } from 'rxjs';
import * as rxjs from 'rxjs';
import { mat3, vec2, vec3 } from 'gl-matrix';

import init_module, * as wasm_bindgen from 'waragraph';

import { placeTooltipAtPoint } from './tooltip';
import { wrapWasmPtr } from './wrap';

import { type PathInterval, type Bp } from './types';

import { Float32, Table, Vector } from 'apache-arrow';
import { Waragraph } from './waragraph';
import { GraphLayoutTable, blobLineIterator } from './graph_layout';

let wasm;
let _raving_ctx;

export type View2DObj = { x: number, y: number, width: number, height: number };

export type OverlayCallback2D =
  (overlay: HTMLCanvasElement, view: wasm_bindgen.View2D, mousePos: { x: number, y: number })
    => void;


interface OverlayCallbacks {
  [callback_key: string]: OverlayCallback2D;
}

export class GraphViewer {
  graph_viewer: wasm_bindgen.GraphViewer;
  graph_layout: GraphLayoutTable;

  initial_view: wasm_bindgen.View2D;

  next_view: wasm_bindgen.View2D;
  view_subject: BehaviorSubject<View2DObj>;

  overlayCallbacks: OverlayCallbacks;
  mousePos: { x: number, y: number } | null;

  gpu_canvas: HTMLCanvasElement;
  overlay_canvas: HTMLCanvasElement;
  container: HTMLDivElement | undefined;

  constructor(
    gpu_canvas: HTMLCanvasElement,
    overlay_canvas: HTMLCanvasElement,
    viewer: wasm_bindgen.GraphViewer,
    graph_layout_table: GraphLayoutTable,
    container?: HTMLDivElement,
  ) {
    this.gpu_canvas = gpu_canvas;
    this.overlay_canvas = overlay_canvas;

    this.container = container;

    // maybe just take the minimum raw data needed here
    this.graph_viewer = viewer;
    this.graph_layout = graph_layout_table;
    // this.segment_positions = seg_pos;

    this.initial_view = this.graph_viewer.get_view();
    this.next_view = this.graph_viewer.get_view();

    this.view_subject = new BehaviorSubject(this.next_view.as_obj());

    this.overlayCallbacks = {};
    this.mousePos = null;
  }

  needRedraw() {
    return !this.next_view.equals(this.graph_viewer.get_view());
  }

  lookup(x: number, y: number): number | null {
    try {
      let val = this.graph_viewer.gbuffer_lookup(_raving_ctx, x, y);
      console.log(val);
      return val;
    } catch (e) {
      return null;
    }
  }

  sampleWorldSpacePath(
    interval: PathInterval,
    tolerance: number,
  ): Promise<Float32Array | undefined> {
    return this.segment_positions?.sample_path(interval, tolerance);
  }

    /*
  sampleCanvasSpacePath(
    path_step_slice: Uint32Array,
    tolerance: number
  ): wasm_bindgen.CanvasPathTrace | undefined {
    const canvas = document.getElementById("graph-viewer-2d") as HTMLCanvasElement;

    return this.segment_positions?.sample_canvas_space_path(
      this.next_view,
      canvas.width,
      canvas.height,
      path_step_slice,
      tolerance
    );
  }
     */

  resetView() {
    let initial_view = this.initial_view.as_obj();

    let canvas = document.getElementById("graph-viewer-2d") as HTMLCanvasElement | null;
    if (!canvas) {
      console.warn("graph-viewer-2d canvas not found");
      return;
    }
    let view_width, view_height;

    let c_aspect = canvas.width / canvas.height;
    let g_aspect = initial_view.width / initial_view.height;

    if (g_aspect > c_aspect) {
      view_width = initial_view.width;
      view_height = view_width * canvas.height / canvas.width;
    } else {
      view_height = initial_view.height;
      view_width = view_height * canvas.width / canvas.height;
    }


    this.next_view.set_center(initial_view.x, initial_view.y);
    this.next_view.set_size(view_width, view_height);
  }

  resize() {
    let el = document.getElementById('graph-viewer-2d-overlay') as HTMLCanvasElement;
    let parent = el.parentNode as HTMLElement;

    let width = parent.clientWidth;
    let height = parent.clientHeight;

    el.width = width;
    el.height = height;

    this.graph_viewer.resize(_raving_ctx, Math.round(width), Math.round(height));
    this.resetView();
  }

  draw() {
    this.graph_viewer.set_view(this.next_view);
    this.graph_viewer.draw_to_surface(_raving_ctx);

    this.view_subject.next(this.next_view.as_obj());

    this.drawOverlays();
  }

  drawOverlays() {
    let overlay = document
      .getElementById('graph-viewer-2d-overlay') as HTMLCanvasElement;
    let ctx = overlay.getContext('2d');

    ctx.clearRect(0, 0, overlay.width, overlay.height);

    // console.log(this.overlayCallbacks);
    for (const key in this.overlayCallbacks) {
      const callback = this.overlayCallbacks[key];
      callback(overlay, this.next_view, this.mousePos);
    }
  }

  registerOverlayCallback(cb_key: string, callback: OverlayCallback2D) {
    this.overlayCallbacks[cb_key] = callback;
    this.drawOverlays();
  }

  removeOverlayCallback(cb_key: string) {
    delete this.overlayCallbacks[cb_key];
    this.drawOverlays();
  }

  // global space
  setViewCenter(x: number, y: number) {
    this.next_view.set_center(x, y);
  }

  // view normalized units
  translate(x: number, y: number) {
    this.next_view.translate_size_rel(x, y);
  }

  // view normalized units
  zoom(tx: number, ty: number, s: number) {
    this.next_view.zoom_with_focus(tx, ty, s);
  }

  getView(): View2DObj {
    return this.next_view.as_obj();
  }

  getViewMatrix(): mat3 {
    let overlay = document
      .getElementById('graph-viewer-2d-overlay') as HTMLCanvasElement;

    return this.graph_viewer.get_view_matrix(overlay.width, overlay.height);
  }

  getSegmentPos(segment: number): { x0: number, y0: number, x1: number, y1: number } | null {
    const pos = this.graph_layout.segmentPosition(segment);
    if (pos === null) {
      return null;
    }
    const { p0, p1 } = pos;
    return { x0: p0[0], y0: p0[1], x1: p1[0], y1: p1[1] };
  }

  getSegmentScreenPos(segment: number): { start: vec2, end: vec2 } | null {
    const pos = this.getSegmentPos(segment);
    if (pos === null) {
      return null;
    }

    const { x0, y0, x1, y1 } = pos;

    const mat = this.getViewMatrix();

    const p0 = vec2.fromValues(x0, y0);
    const p1 = vec2.fromValues(x1, y1);

    const q0 = vec2.create();
    const q1 = vec2.create();

    vec2.transformMat3(q0, p0, mat);
    vec2.transformMat3(q1, p1, mat);

    return { start: q0, end: q1 };
  }
}

let _wasm;

let pathHighlightTolerance = 5;

export function getPathTolerance() {
  return pathHighlightTolerance;
}

export function setPathTolerance(tol) {
  pathHighlightTolerance = tol;
}

// input ranges should be in path space
export function preparePathHighlightOverlay(seg_pos, path_steps, path_cs_raw, entries) {
  const path_cs = wrapWasmPtr(wasm_bindgen.CoordSys, path_cs_raw.__wbg_ptr);

  const processed = [];

  for (const entry of entries) {
    const { start, end, label } = entry;
    const step_range = path_cs.bp_to_step_range(BigInt(start), BigInt(end));
    const path_slice = path_steps.slice(step_range.start, step_range.end);

    processed.push({ path_slice, color: entry.color, start, end, label });
    console.log("steps in ", entry.label, ": ", path_slice.length);
  }

  // console.log(processed);

  return (canvas, view, mouse_pos) => {

    /*
    {
        let ctx = canvas.getContext('2d');
        ctx.save();

        // const path = new Path2D();
        // path.ellipse(150, 75, 40, 60, Math.PI * 0.25, 0, 2 * Math.PI);
        // ctx.strokeRect(0, 0, 300, 300);
        // ctx.stroke(path);
        ctx.globalAlpha = 1.0;
        ctx.fillStyle = 'black';
        ctx.fillRect(0, 0, 300, 300);

        ctx.restore();
    }
    */

    let view_matrix = view.to_js_mat3(canvas.width, canvas.height);
    // console.log(view_matrix);

    let ctx = canvas.getContext('2d');
    ctx.save();

    for (const entry of processed) {

      try {
        let canv_path = seg_pos.sample_canvas_space_path(
          view,
          canvas.width,
          canvas.height,
          entry.path_slice,
          pathHighlightTolerance,
        )

        // TODO handle zero length path cases
        let len = canv_path.length;
        // console.warn("canvas path length: ", len);

        ctx.beginPath();

        let start = canv_path.get_point(0);
        ctx.moveTo(start.x, start.y);

        canv_path.with_points((x, y) => {
          ctx.lineTo(x, y);
        });

        ctx.globalAlpha = 0.8;
        // ctx.globalCompositeOperation = "copy";
        ctx.lineWidth = 15;
        ctx.strokeStyle = entry.color;
        ctx.stroke();

        /*
        if (ctx.isPointinStroke(mouse_pos.x, mouse_pos.xy)) {
            console.log(entry.label);
            // tooltip.innerHTML = `Segment ${segment}`;
            // tooltip.style.display = 'block';
            // placeTooltipAtPoint(x, y);
        }
        */
        ctx.closePath();

        // ctx.strokeStyle = 'black';
        // ctx.fillStyle = 'black';

        let ends = canv_path.get_endpoints();

        if (ends !== null) {
          // console.warn(ends);

          let x = ends.start.x + (ends.end.x - ends.start.x) * 0.5;
          let y = ends.start.y + (ends.end.y - ends.start.y) * 0.5;
          ctx.fillText(entry.label, x, y);
        }

      } catch (e) {
        console.error("oh no: ", e);
        //
      }


    }

    ctx.restore();
  };
}


/*
function resize_view_dimensions(v_dims, c_old, c_new) {
  let [v_w, v_h] = v_d;
  let [c_old_w, c_old_h] = c_old;
  let [c_new_w, c_new_h] = c_new;

  let S_w = c_new_w / c_old_w;
  let S_h = c_new_h / c_old_h;
  let S = Math.min(S_w, S_h);

  let v_new_w = v_w * S;
  let v_new_h = v_h * S;

  return [v_new_h, v_new_h];
}
  */




export async function graphViewerFromData(
  container: HTMLDivElement,
  layout_data: GraphLayoutTable,
  color_data?: Uint32Array,
): Promise<GraphViewer> {
  const wasm = await init_module();
  wasm_bindgen.set_panic_hook();

  let gpu_canvas = document.createElement('canvas');
  let overlay_canvas = document.createElement('canvas');

  gpu_canvas.id = 'graph-viewer-2d';
  overlay_canvas.id = 'graph-viewer-2d-overlay';

  gpu_canvas.style.setProperty('position', 'absolute');
  overlay_canvas.style.setProperty('position', 'absolute');
  gpu_canvas.style.setProperty('z-index', '0');
  overlay_canvas.style.setProperty('z-index', '1');

  container.append(gpu_canvas);
  container.append(overlay_canvas);

  gpu_canvas.width = container.clientWidth;
  gpu_canvas.height = container.clientHeight;
  overlay_canvas.width = container.clientWidth;
  overlay_canvas.height = container.clientHeight;

  if (!_raving_ctx) {
    _raving_ctx = await wasm_bindgen.RavingCtx.initialize_(gpu_canvas);
  }


  const position_buffers = _raving_ctx.create_empty_paged_buffers(20n);

  let graph_bounds: { min_x: any; max_x: any; min_y: any; max_y: any; };
  {
    await fillPositionBuffersFromArrowVectors(_raving_ctx, position_buffers, layout_data.x, layout_data.y);
    const [min_x, min_y] = layout_data.aabb_min;
    const [max_x, max_y] = layout_data.aabb_max;
    graph_bounds = { min_x, min_y, max_x, max_y };
  }

  const segment_count = position_buffers.len();
  if (segment_count === 0) {
    throw new Error("parsed 0 segment positions");
  }
  console.warn(`parsed ${segment_count} segment positions`);

  const color_buffers = _raving_ctx.create_paged_buffers(4n, segment_count);
  const color_page_cap = color_buffers.page_capacity();

  console.warn("uploading color data...");

  if (color_data === undefined) {
      const col_buf_f_array = new Uint32Array(color_page_cap);
      col_buf_f_array.fill(0xFF0000FF);

      for (let page_ix = 0; page_ix < color_buffers.page_count(); page_ix++) {
        let col_buf_array = new Uint8Array(col_buf_f_array.buffer);
        color_buffers.upload_page(_raving_ctx, page_ix, col_buf_array);
      }

      color_buffers.set_len(segment_count);
  } else {

    for (let page_ix = 0; page_ix < color_buffers.page_count(); page_ix++) {
      const start = page_ix * color_page_cap;
      const end = start + color_page_cap;
      const color_page = color_data.subarray(start, end);
      const page = new Uint8Array(color_page.buffer);

      color_buffers.upload_page(_raving_ctx, page_ix, page);
    }

    color_buffers.set_len(segment_count);
  }

  console.warn("color data uploaded");


  let { min_x, max_x, min_y, max_y } = graph_bounds;
  let g_width = max_x - min_x;
  let g_height = max_y - min_y;

  console.warn(graph_bounds);

  let view = wasm_bindgen.View2D.new_center_size(
    min_x + g_width / 2,
    min_y + g_height / 2,
    g_width,
    g_height,
  );

  const viewer = wasm_bindgen.GraphViewer.new_with_buffers(_raving_ctx, position_buffers, color_buffers, gpu_canvas, view);

  console.warn(container.clientWidth, ", ", container.clientHeight);
  viewer.resize(_raving_ctx, container.clientWidth, container.clientHeight);

  viewer.draw_to_surface(_raving_ctx);

  const graph_viewer = new GraphViewer(gpu_canvas, overlay_canvas, viewer, layout_data, container);

  const draw_loop = () => {
    if (graph_viewer.needRedraw()) {
      graph_viewer.draw();
    }

    window.requestAnimationFrame(draw_loop);
  };

  draw_loop();

  const mouseDown$ = rxjs.fromEvent(overlay_canvas, 'mousedown');
  const mouseUp$ = rxjs.fromEvent(overlay_canvas, 'mouseup');
  const mouseOut$ = rxjs.fromEvent(overlay_canvas, 'mouseout');
  const mouseMove$ = rxjs.fromEvent<MouseEvent>(overlay_canvas, 'mousemove');


  const hoveredSegment$ = mouseMove$.pipe(
    rxjs.map((ev: MouseEvent) => ({ x: ev.offsetX, y: ev.offsetY })),
    rxjs.distinct(),
    rxjs.throttleTime(40),
    rxjs.map(({ x, y }) => graph_viewer.lookup(x, y)),
  );

  /*
  hoveredSegment$.subscribe((segment) => {
    let tooltip = document.getElementById('tooltip');

    if (segment === null) {
      tooltip.innerHTML = "";
      tooltip.style.display = 'none';
    } else if (graph_viewer.mousePos !== null) {
      tooltip.innerHTML = `Segment ${segment}`;
      tooltip.style.display = 'block';

      let rect = document
        .getElementById('graph-viewer-2d-overlay')
        .getBoundingClientRect();
      let { x, y } = graph_viewer.mousePos;

      let gx = x + rect.left;
      let gy = y + rect.top;

      placeTooltipAtPoint(gx, gy);
    }
  })
    */

  mouseMove$.subscribe((event: MouseEvent) => {
    graph_viewer.mousePos = { x: event.offsetX, y: event.offsetY };
  });

  mouseOut$.subscribe((event) => {
    graph_viewer.mousePos = null;
  });

  const drag$ = mouseDown$.pipe(
    rxjs.switchMap((event) => {
      return mouseMove$.pipe(
        rxjs.takeUntil(
          rxjs.race(mouseUp$, mouseOut$)
        )
      )
    })
  );

  drag$.subscribe((ev: MouseEvent) => {
    let dx = ev.movementX;
    let dy = ev.movementY;
    let x = dx / overlay_canvas.width;
    let y = dy / overlay_canvas.height;
    graph_viewer.translate(-x, y);
  });

  const wheel$ = rxjs.fromEvent(overlay_canvas, 'wheel').pipe(
    rxjs.tap(event => event.preventDefault())
  );

  wheel$.subscribe((event: WheelEvent) => {
    let x = event.offsetX;
    let y = overlay_canvas.height - event.offsetY;

    let nx = x / overlay_canvas.width;
    let ny = y / overlay_canvas.height;

    let scale = event.deltaY > 0.0 ? 1.05 : 0.95;

    graph_viewer.zoom(nx, ny, scale);
  });

  graph_viewer.resetView();


  return graph_viewer;

}


async function fillPositionBuffersFromTSV(
  raving_ctx: wasm_bindgen.RavingCtx,
  buffers: wasm_bindgen.PagedBuffers,
  position_tsv: Blob,
): Promise<{ min_x: number; max_x: number; min_y: number; max_y: number; }> {
  let position_lines = blobLineIterator(position_tsv);

  let header = await position_lines.next();

  const regex = /([^\s]+)\t([^\s]+)\t([^\s]+)/;

  const parse_next = async () => {
    let line = await position_lines.next();

    let match = line.value?.match(regex);
    if (!match) {
      return null;
    }

    // let ix = parseInt(match[1]);
    let x = parseFloat(match[2]);
    let y = parseFloat(match[3]);

    return { x, y };
  }

  let page_byte_size = buffers.page_size();
  let page_buffer = new ArrayBuffer(Number(page_byte_size));
  let page_view = new DataView(page_buffer);

  let seg_i = 0;
  let offset = 0;

  let min_x = Infinity;
  let min_y = Infinity;
  let max_x = -Infinity;
  let max_y = -Infinity;

  for (;;) {
    const p0 = await parse_next();
    const p1 = await parse_next();

    if (p0 === null || p1 === null) {
      break;
    }

    min_x = Math.min(min_x, p0.x, p1.x);
    min_y = Math.min(min_y, p0.y, p1.y);

    max_x = Math.max(max_x, p0.x, p1.x);
    max_y = Math.max(max_y, p0.y, p1.y);

    page_view.setFloat32(offset, p0.x, true);
    page_view.setFloat32(offset + 4, p0.y, true);
    page_view.setFloat32(offset + 8, p1.x, true);
    page_view.setFloat32(offset + 12, p1.y, true);
    page_view.setUint32(offset + 16, seg_i, true);

    seg_i += 1;
    offset += 20;

    if (offset >= page_byte_size) {
      console.warn(`appending page, offset ${offset}`);
      buffers.append_page(raving_ctx, new Uint8Array(page_buffer, 0, offset));
      offset = 0;
    }
  }

  if (offset !== 0) {
      console.warn(`closing with appending page, offset ${offset}`);
      buffers.append_page(raving_ctx, new Uint8Array(page_buffer, 0, offset));
  }

  return { min_x, min_y, max_x, max_y };
}


async function fillPositionBuffersFromArrowVectors(
  raving_ctx: wasm_bindgen.RavingCtx,
  buffers: wasm_bindgen.PagedBuffers,
  x: Vector<Float32>,
  y: Vector<Float32>,
  // positions: Table,
): Promise<{ min_x: number; max_x: number; min_y: number; max_y: number; }> {

  let page_byte_size = buffers.page_size();
  let page_buffer = new ArrayBuffer(Number(page_byte_size));
  let page_view = new DataView(page_buffer);

  const x_iter = x[Symbol.iterator]();
  const y_iter = y[Symbol.iterator]();

  const get_next = () => {
    let res_x = x_iter.next();
    let res_y = y_iter.next();

    if (res_x.done || res_y.done) {
      return null;
    }

    let x = res_x.value;
    let y = res_y.value;

    return { x, y };
  };


  let min_x = Infinity;
  let min_y = Infinity;
  let max_x = -Infinity;
  let max_y = -Infinity;

  let seg_i = 0;
  let offset = 0;

  for (;;) {
    const p0 = get_next();
    const p1 = get_next();

    if (p0 == null || p1 == null) {
      break;
    }

    min_x = Math.min(min_x, p0.x, p1.x);
    min_y = Math.min(min_y, p0.y, p1.y);

    max_x = Math.max(max_x, p0.x, p1.x);
    max_y = Math.max(max_y, p0.y, p1.y);

    page_view.setFloat32(offset, p0!.x, true);
    page_view.setFloat32(offset + 4, p0!.y, true);
    page_view.setFloat32(offset + 8, p1!.x, true);
    page_view.setFloat32(offset + 12, p1!.y, true);
    page_view.setUint32(offset + 16, seg_i, true);

    seg_i += 1;
    offset += 20;

    if (offset >= page_byte_size) {
      console.warn(`appending page, offset ${offset}`);
      buffers.append_page(raving_ctx, new Uint8Array(page_buffer, 0, offset));
      offset = 0;
    }
  }

  if (offset !== 0) {
      console.warn(`closing with appending page, offset ${offset}`);
      buffers.append_page(raving_ctx, new Uint8Array(page_buffer, 0, offset));
  }

  return { min_x, max_x, min_y, max_y };
}



