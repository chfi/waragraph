import { Table, makeTable } from "apache-arrow";
import { vec2 } from "gl-matrix";




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

  const xs = [];
  const ys = [];

  for (;;) {
    const row = await parse_next();
    if (row === null) {
      break;
    }

    const { x, y } = row;
    xs.push(x);
    ys.push(y);
  }

  const table = makeTable({
    x: Float32Array.from(xs),
    y: Float32Array.from(ys)
  });

  return new GraphLayoutTable(table);
}

export class GraphLayoutTable {
  table: Table;

  constructor(table: Table) {
    // TODO check fields
    this.table = table;
  }

  segmentPosition(segment: number): { p0: vec2, p1: vec2 } | null {
    if (!Number.isInteger(segment)) {
      return null;
    }

    let s0 = segment << 1;
    let s1 = s0 + 1;

    let q0 = this.table.get(s0);
    let q1 = this.table.get(s1);

    if (q0 === null || q1 === null) {
      return null;
    }

    let p0 = vec2.fromValues(q0['x'], q0['y']);
    let p1 = vec2.fromValues(q1['x'], q1['y']);

    return { p0, p1 };
  }

  sample2DPath(path: Uint32Array, tolerance: number): Float32Array {
    const points = new Float32Array(path.length * 2);

    let step_count = 0;
    let added = 0;

    let last_point: vec2 | null = null;

    for (const handle of path) {
      const pos = this.table.get(handle);

      const i = step_count * 2;

      const p = points.subarray(i, i + 2) as vec2;

      const dist = last_point === null ? Infinity : vec2.dist(p, last_point);

      if (dist > tolerance) {
        last_point = p;
        points[i] = pos['x'];
        points[i + 1] = pos['y'];
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
    return new SegmentPositionIterator(this.table);
  }
}


class SegmentPositionIterator implements Iterable<{ segment: number, p0: vec2, p1: vec2 }> {
  table: Table;

  constructor(table: Table) {
    this.table = table;
  }

  [Symbol.iterator](): Iterator<{ segment: number, p0: vec2; p1: vec2; }, any, undefined> {

    const iter = this.table[Symbol.iterator]();

    let nextSegment = 0;

    return {
      next: () => {
        let row0 = iter.next();
        let row1 = iter.next();

        if (row0.done || row1.done) {
          return { value: null, done: true };
        }

        let p0 = vec2.fromValues(row0.value['x'], row0.value['y']);
        let p1 = vec2.fromValues(row1.value['x'], row1.value['y']);
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
