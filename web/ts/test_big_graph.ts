import init_module, * as wasm_bindgen from 'waragraph';

import { vec2 } from 'gl-matrix';

// const MIN_POS = -10000000;
// const MAX_POS = 10000000;

const MIN_POS = -10000;
const MAX_POS = 10000;

// const MIN_POS = -1000;
// const MAX_POS = 1000;

function generatePositionVertex(out_view: DataView, i: number) {

  const cx = MIN_POS + Math.random() * (MAX_POS - MIN_POS);
  const cy = MIN_POS + Math.random() * (MAX_POS - MIN_POS);

  const c = vec2.fromValues(cx, cy);

  const length = 5.0 + Math.random() * 30.0;
  const angle = Math.random() * 2 * Math.PI;

  const dx = length * 0.5 * Math.cos(angle);
  const dy = length * 0.5 * Math.sin(angle);

  const d = vec2.fromValues(dx, dy)

  try {

    const p0 = vec2.create();
    const p1 = vec2.create();

    vec2.sub(p0, c, d);
    vec2.add(p1, c, d);

    out_view.setFloat32(0, p0[0], true);
    out_view.setFloat32(4, p0[1], true);
    out_view.setFloat32(8, p1[0], true);
    out_view.setFloat32(12, p1[1], true);
    out_view.setUint32(16, i, true);

  } catch (e) {
    console.error(e);
    // console.error("oh no!");
    // console.error(out_slice);
  }

}

function generatePositionPage(buffer: ArrayBuffer, page: number) {
  const count = buffer.byteLength / 20;

  // const view = new DataView(buffer);
  // console.warn(new Float32Array(buffer));

  for (let i = 0; i < count; i++) {
    let begin = i * 20;
    let end = begin + 20;
    let view = new DataView(buffer, begin, 20);
    generatePositionVertex(view, page * count + i);

  }

  // console.warn(new Float32Array(buffer));

}

export async function testGen() {
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


  // const element_count = 128000000;
  // const element_count = 12800000 * 4;
  const element_count = 1280000;

  const position_buffers = raving_ctx.create_paged_buffers(20n, element_count);
  const color_buffers = raving_ctx.create_paged_buffers(4n, element_count);

  console.warn("created buffers");

  const pos_page_cap = position_buffers.page_capacity();
  const col_page_cap = color_buffers.page_capacity();

  console.warn("position page capacity (elements): ", pos_page_cap);
  console.warn("color page capacity (elements): ", col_page_cap);

  {
    // const pos_buf_array = new Uint8Array(pos_page_cap);
    // const pos_buf_array = new Uint8Array(pos_page_cap * 20);
    const pos_buf = new ArrayBuffer(pos_page_cap * 20);
    const pos_buf_view = new DataView(pos_buf);

    console.warn("generating position data...");
    for (let page_ix = 0; page_ix < position_buffers.page_count(); page_ix++) {
      console.warn("  page ", page_ix);
      // console.warn("???, ", pos_buf_array);
      generatePositionPage(pos_buf, page_ix);
      // console.warn(`${pos_buf_array[0]}, ${pos_buf_array[1]}, ${pos_buf_array[2]}, ${pos_buf_array[3]}`);
      // console.warn(pos_buf_array[0]);
      // console.warn(pos_buf_array);

      position_buffers.upload_page(raving_ctx, page_ix, new Uint8Array(pos_buf));
    }

    position_buffers.set_len(element_count);

    console.warn("position data uploaded");
  }

  {
    const col_buf_f_array = new Uint32Array(col_page_cap);
    col_buf_f_array.fill(0xFF0000FF);

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
  // let view = wasm_bindgen.View2D.new_center_size(0, 0, MAX_POS * 2, MAX_POS * 2);
  // let view = wasm_bindgen.View2D.new_center_size(0, 0, 10000, 10000);

  console.warn("initializing graph viewer");

  const graph_viewer = wasm_bindgen.GraphViewer.new_with_buffers(raving_ctx, position_buffers, color_buffers, gpu_canvas, view);

  console.warn("drawing graph viewer");

  graph_viewer.draw_to_surface(raving_ctx);

  console.warn("done");

  window.setTimeout(() => 
    graph_viewer.draw_to_surface(raving_ctx),
    500);

}


// adapted from https://developer.mozilla.org/en-US/docs/Web/API/ReadableStreamDefaultReader/read
async function* blobLineIterator(blob: Blob) {
  const utf8Decoder = new TextDecoder("utf-8");
  // let response = await fetch(fileURL);
  // let reader = response.body.getReader();
  let stream = blob.stream();
  let reader = stream.getReader();

  let { value: chunk, done: readerDone } = await reader.read();

  let chunk_str = chunk ? utf8Decoder.decode(chunk, { stream: true }) : "";
  // chunk = chunk ? utf8Decoder.decode(chunk, { stream: true }) : "";

  let re = /\r\n|\n|\r/gm;
  let startIndex = 0;

  for (;;) {
    let result = re.exec(chunk_str);
    if (!result) {
      if (readerDone) {
        break;
      }
      let remainder = chunk_str.substring(startIndex);
      ({ value: chunk, done: readerDone } = await reader.read());
      chunk_str =
        remainder + (chunk ? utf8Decoder.decode(chunk, { stream: true }) : "");
      startIndex = re.lastIndex = 0;
      continue;
    }
    yield chunk_str.substring(startIndex, result.index);
    startIndex = re.lastIndex;
  }
  if (startIndex < chunk_str.length) {
    // last line didn't end in a newline char
    yield chunk_str.substring(startIndex);
  }
}


async function fillPositionPagedBuffers(
  raving_ctx: wasm_bindgen.RavingCtx,
  buffers: wasm_bindgen.PagedBuffers,
  position_tsv: Blob,
): { min_x: number, max_x: number, min_y: number, max_y: number } {
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


export async function testBig(layout_file: File) {
  const wasm = await init_module();
  wasm_bindgen.set_panic_hook();

  const gpu_canvas = document.createElement('canvas');

  document.body.append(gpu_canvas);

  const raving_ctx = await wasm_bindgen.RavingCtx.initialize_(gpu_canvas);

  const position_buffers = raving_ctx.create_empty_paged_buffers(20n);

  const graph_bounds = await fillPositionPagedBuffers(raving_ctx, position_buffers, layout_file);

  const segment_count = position_buffers.len();
  console.warn(`parsed ${segment_count} segment positions`);

  const color_buffers = raving_ctx.create_paged_buffers(4n, segment_count);
  const col_page_cap = color_buffers.page_capacity();

  {
    const col_buf_f_array = new Uint32Array(col_page_cap);
    col_buf_f_array.fill(0xFF0000FF);

    console.warn("uploading color data...");
    for (let page_ix = 0; page_ix < color_buffers.page_count(); page_ix++) {
      console.warn("  page ", page_ix);
      let col_buf_array = new Uint8Array(col_buf_f_array.buffer);
      color_buffers.upload_page(raving_ctx, page_ix, col_buf_array);
    }

    color_buffers.set_len(segment_count);

    console.warn("color data uploaded");
  }


  let { min_x, max_x, min_y, max_y } = graph_bounds;
  let width = max_x - min_x;
  let height = max_y - min_y;

  console.warn(graph_bounds);

  let view = wasm_bindgen.View2D.new_center_size(
    min_x + width / 2,
    min_y + height / 2,
    width,
    height,
  );

  const graph_viewer = wasm_bindgen.GraphViewer.new_with_buffers(raving_ctx, position_buffers, color_buffers, gpu_canvas, view);

  graph_viewer.draw_to_surface(raving_ctx);

  console.warn("done");

  window.setTimeout(() => {
    graph_viewer.draw_to_surface(raving_ctx);
    console.warn("done again");
  }, 500);
}
