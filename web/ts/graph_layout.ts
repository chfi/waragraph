import { Table, Vector, makeTable, makeVector } from "apache-arrow";
import { vec2 } from "gl-matrix";


export class GraphLayoutTable {
  x: Vector;
  y: Vector;

  aabb_min: vec2;
  aabb_max: vec2;

  constructor(x: Vector | Float32Array, y: Vector | Float32Array, aabb_min: vec2, aabb_max: vec2) {
    if (x instanceof Vector) {
      this.x = x;
    } else {
      this.x = makeVector(x);
    }
    if (y instanceof Vector) {
      this.y = y;
    } else {
      this.y = makeVector(y);
    }
    this.aabb_min = aabb_min;
    this.aabb_max = aabb_max;
  }

  endpointPosition(endpoint: number): vec2 | null {
    const x = this.x.get(endpoint);
    const y = this.y.get(endpoint);
    return vec2.fromValues(x, y);
  }

  segmentPosition(segment: number): { p0: vec2, p1: vec2 } | null {
    if (!Number.isInteger(segment)) {
      return null;
    }

    let s0 = segment << 1;
    let s1 = s0 + 1;

    let q0 = this.endpointPosition(s0);
    let q1 = this.endpointPosition(s1);

    if (q0 === null || q1 === null) {
      return null;
    }

    let p0 = vec2.fromValues(q0[0], q0[1]);
    let p1 = vec2.fromValues(q1[0], q1[1]);

    return { p0, p1 };
  }

  sample2DPath(path: Uint32Array, tolerance: number): Float32Array {
    const points = new Float32Array(path.length * 2);

    let step_count = 0;
    let added = 0;

    let last_point: vec2 | null = null;

    for (const handle of path) {
      const pos = this.endpointPosition(handle);

      const i = step_count * 2;

      const p = points.subarray(i, i + 2) as vec2;

      const dist = last_point === null ? Infinity : vec2.dist(p, last_point);

      if (dist > tolerance) {
        last_point = p;
        points[i] = pos[0];
        points[i + 1] = pos[1];
        added += 1;
      }

      step_count += 1;

    }

    const out_buffer = new ArrayBuffer(added * 2 * 4);
    const out = new Float32Array(out_buffer);
    out.set(points.subarray(0, added * 2));

    return out;
  }

  iterateSegments(): Iterable<{ segment: number, p0: vec2, p1: vec2 }> {
    return new SegmentPositionIterator(this.x, this.y);
  }
}


export async function graphLayoutFromTSV(
  tsv_file: Blob,
): Promise<GraphLayoutTable> {
  let position_lines = blobLineIterator(tsv_file);

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

  const xs: number[] = [];
  const ys: number[] = [];

  let x_min = Infinity;
  let y_min = Infinity;
  let x_max = -Infinity;
  let y_max = -Infinity;

  for (;;) {
    const row = await parse_next();
    if (row === null) {
      break;
    }

    const { x, y } = row;
    xs.push(x);
    ys.push(y);

    x_min = Math.min(x_min, x);
    y_min = Math.min(y_min, y);
    x_max = Math.max(x_max, x);
    y_max = Math.max(y_max, y);
  }

  const x = makeVector(Float32Array.from(xs));
  const y = makeVector(Float32Array.from(ys));

  return new GraphLayoutTable(x, y, vec2.fromValues(x_min, y_min), vec2.fromValues(x_max, y_max));
}



class SegmentPositionIterator implements Iterable<{ segment: number, p0: vec2, p1: vec2 }> {
  // table: Table;
  x: Vector;
  y: Vector;

  // constructor(table: Table) {
    // this.table = table;
  constructor(x: Vector, y: Vector) {
    this.x = x;
    this.y = y;
  }

  [Symbol.iterator](): Iterator<{ segment: number, p0: vec2; p1: vec2; }, any, undefined> {

    const x_iter = this.x[Symbol.iterator]();
    const y_iter = this.y[Symbol.iterator]();
    // const iter = this.table[Symbol.iterator]();

    let nextSegment = 0;

    return {
      next: () => {
        let x0 = x_iter.next();
        let y0 = y_iter.next();
        let x1 = x_iter.next();
        let y1 = y_iter.next();

        if (x0.done || y0.done || y1.done || x1.done) {
          return { value: null, done: true };
        }

        let p0 = vec2.fromValues(x0.value, y0.value);
        let p1 = vec2.fromValues(x1.value, y1.value);
        let segment = nextSegment;
        nextSegment += 1;

        return { value: { segment, p0, p1 }, done: false };
      }
    }

  }
}



export async function* blobLineIterator(blob: Blob) {
  const utf8Decoder = new TextDecoder("utf-8");
  let stream = blob.stream();
  let reader = stream.getReader();

  let { value: chunk, done: readerDone } = await reader.read();

  let chunk_str = chunk ? utf8Decoder.decode(chunk, { stream: true }) : "";

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
