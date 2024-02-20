

import { DataType, Table, Vector, makeTable, makeVector } from 'apache-arrow';
import { TypedArray } from 'apache-arrow/interfaces';

export interface CoordSysInterface {

  max(): number;
  segmentAtPosition(bp: BigInt): number | null;
  segmentOffset(segment: number): number | null;
  segmentRange(segment: number): {start: number, end: number} | null;
  // segment_at_pos(bp: BigInt): Promise<number | null>;
  // offset_at(segment: number): Promise<number | null>;
  // segment_range(segment: number): Promise<{start: number, end: number} | null>;

}


export class CoordSysArrow {
  // table: Table;
  node_order: Vector;
  step_offsets: Vector;

  // constructor(table: Table) {
  //   this.table = table;
  // }

  constructor(node_order: Vector | Uint32Array, step_offsets: Vector | Int32Array) {
    if (node_order instanceof Vector) {
      this.node_order = node_order;
    } else {
      this.node_order = makeVector(node_order);
    }

    if (step_offsets instanceof Vector) {
      this.step_offsets = step_offsets;
    } else {
      this.step_offsets = makeVector(step_offsets);
    }
  }

  max(): number {
    return this.step_offsets.get(this.step_offsets.length - 1);
  }

  segmentAtPosition(bp: BigInt): number | null {
    const offsets = this.step_offsets;
    const pos_i = binarySearch(offsets.data[0].values, Number(bp));
    if (pos_i >= offsets.length) {
      return null;
    }
    return pos_i;
  }

  segmentOffset(segment: number): number | null {
    return this.step_offsets.get(segment);
  }

  segmentRange(segment: number): { start: number, end: number } | null {
    const offsets = this.step_offsets;
    const start = offsets.get(segment);
    const end = offsets.get(segment + 1);

    if (start === null || end === null) {
      return null;
    }
    return { start, end };
  }

}



export function coordSysFromBuffers(
  node_order_buf: SharedArrayBuffer,
  step_offsets_buf: SharedArrayBuffer,
): CoordSysArrow  {

  const node_order = new Uint32Array(node_order_buf);
  const step_offsets = new Int32Array(step_offsets_buf);

  return new CoordSysArrow(node_order, step_offsets);
}

export function coordSysFromTable(
  table: Table,
): CoordSysArrow {
  const node_order = table.getChild('node_order')!;
  const step_offsets = table.getChild('step_offsets')!;

  return new CoordSysArrow(node_order, step_offsets);
}


function binarySearch<T extends TypedArray>(arr: T, target: number): number {
    let left = 0;
    let right = arr.length - 1;

    while (left <= right) {
        const mid = Math.floor((left + right) / 2);

        if (arr[mid] === target) {
            return mid; // Target found
        } else if (arr[mid] < target) {
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }

    // Target not found, return the index where it can be inserted
    return left;
}
