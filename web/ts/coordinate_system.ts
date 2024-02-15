

import { DataType, Table, Vector } from 'apache-arrow';
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
  table: Table;

  constructor(table: Table) {
    this.table = table;
  }

  max(): number {
    const offsets = this.table.getChild('step_offsets')!;
    return offsets.get(offsets.length - 1);
  }

  segmentAtPosition(bp: BigInt): number | null {
    const offsets = this.table.getChild('step_offsets')!;
    const pos_i = binarySearch(offsets.data[0].values, Number(bp));
    if (pos_i >= offsets.length) {
      return null;
    }
    return pos_i;
  }

  segmentOffset(segment: number): number | null {
    const offsets = this.table.getChild('step_offsets')!;
    return offsets.get(segment);
  }

  segmentRange(segment: number): { start: number, end: number } | null {
    const offsets = this.table.getChild('step_offsets')!;
    const start = offsets.get(segment);
    const end = offsets.get(segment + 1);

    if (start === null || end === null) {
      return null;
    }
    return { start, end };
  }

}



export async function coordSysFromTable(
  table: Table,
){
  return new CoordSysArrow(table);
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
