import init_module, * as wasm_bindgen from 'waragraph';

import type { WaragraphWorkerCtx, PathViewerCtx } from './worker';

import { initializePathViewer, addOverviewEventHandlers, addPathViewerLogic } from './path_viewer_ui';
import type { PathViewer } from './path_viewer_ui';
import { OverviewMap } from './overview';

import * as CanvasTracks from './canvas_tracks';
import * as BedSidebar from './sidebar-bed';

import type { Bp, Segment, Handle, PathId, RGBAObj, RGBObj } from './types';

import { vec2 } from 'gl-matrix';

const MIN_POS = -10000000;
const MAX_POS = 10000000;

function generatePositionVertex(out_slice: ArrayBuffer, i: number) {

  const cx = Math.random() * (MAX_POS - MIN_POS);
  const cy = Math.random() * (MAX_POS - MIN_POS);

  const c = vec2.fromValues(cx, cy);

  const length = 5.0 + Math.random() * 30.0;
  const angle = Math.random() * 2 * Math.PI;

  const dx = length * 0.5 * Math.cos(angle);
  const dy = length * 0.5 * Math.sin(angle);

  const d = vec2.fromValues(dx, dy)

  try {

    const p0 = new Float32Array(out_slice, 0, 2);
    const p1 = new Float32Array(out_slice, 2 * 4, 2);
    const id = new Uint32Array(out_slice, 4 * 4, 1);

    vec2.sub(p0, c, d);
    vec2.add(p1, c, d);

    id[0] = i;

  } catch (e) {
    console.error(out_slice);

  }

}

function generatePositionPage(buffer: ArrayBuffer, page: number) {
  const count = buffer.byteLength / 20;

  for (let i = 0; i < count; i++) {
    let begin = i * 20;
    let end = begin + 20;
    let slice = buffer.slice(begin, end);
    generatePositionVertex(slice, page * count + i);
  }

}

export async function testBig() {

  const wasm = await init_module();
  wasm_bindgen.set_panic_hook();

  const gpu_canvas = document.createElement('canvas');

  document.body.append(gpu_canvas);

  const raving_ctx = await wasm_bindgen.RavingCtx.initialize_(gpu_canvas);

  // wasm is set to 512MB memory limit in .cargo/config.toml,
  // and the webgl2 limits set the maximum buffer size to 256MB

  // the 2D viewer needs two sets of buffers, one with positions and IDs,
  // another with 32-bit colors
  // the first has a stride of 4*5 = 20 bytes, the second 4 bytes

  // the position buffer then fits 12 800 000 elements per page, and the
  // color buffer 64 000 000 elements

  // let's go well beyond the 512MB limit and generate 10 pages, or 128 million
  // elements -- the color data can be static, and the positions should be contained
  // in a box of some size


  const element_count = 128000000;
  // const element_count = 12800000 * 4;

  const position_buffers = raving_ctx.create_paged_buffers(20n, element_count);
  const color_buffers = raving_ctx.create_paged_buffers(4n, element_count);

  console.warn("created buffers");

  const pos_page_cap = position_buffers.page_capacity();
  const col_page_cap = color_buffers.page_capacity();

  console.warn("position page capacity (elements): ", pos_page_cap);
  console.warn("color page capacity (elements): ", col_page_cap);

  {
    const pos_buf_array = new Uint8Array(pos_page_cap);

    console.warn("generating position data...");
    for (let page_ix = 0; page_ix < position_buffers.page_count(); page_ix++) {
      console.warn("  page ", page_ix);
      generatePositionPage(pos_buf_array, page_ix);
      position_buffers.upload_page(raving_ctx, page_ix, pos_buf_array);
    }

    position_buffers.set_len(element_count);

    console.warn("position data uploaded");
  }

  {
    const col_buf_f_array = new Float32Array(col_page_cap);
    col_buf_f_array.fill(0xFFAAAAFF);

    console.warn("uploading color data...");
    for (let page_ix = 0; page_ix < color_buffers.page_count(); page_ix++) {
      console.warn("  page ", page_ix);
      let col_buf_array = new Uint8Array(col_buf_f_array.buffer);
      color_buffers.upload_page(raving_ctx, page_ix, col_buf_array);
    }

    color_buffers.set_len(element_count);

    console.warn("position data uploaded");
  }

  let view = wasm_bindgen.View2D.new_center_size(0, 0, MAX_POS / 5, MAX_POS / 5);

  console.warn("initializing graph viewer");

  const graph_viewer = wasm_bindgen.GraphViewer.new_with_buffers(raving_ctx, position_buffers, color_buffers, gpu_canvas, view);

  console.warn("drawing graph viewer");

  graph_viewer.draw_to_surface(raving_ctx);

}
