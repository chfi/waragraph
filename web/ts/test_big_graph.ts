import init_module, * as wasm_bindgen from 'waragraph';

import type { WaragraphWorkerCtx, PathViewerCtx } from './worker';

import { initializePathViewer, addOverviewEventHandlers, addPathViewerLogic } from './path_viewer_ui';
import type { PathViewer } from './path_viewer_ui';
import { OverviewMap } from './overview';

import * as CanvasTracks from './canvas_tracks';
import * as BedSidebar from './sidebar-bed';

import type { Bp, Segment, Handle, PathId, RGBAObj, RGBObj } from './types';

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

  const position_buffers = raving_ctx.create_paged_buffers(20n, element_count);
  const color_buffers = raving_ctx.create_paged_buffers(5n, element_count);

  console.warn("created buffers");

}
