

import { Table } from 'apache-arrow';

export interface CoordSysInterface {

  max_f64(): number;
  segment_at_pos(bp: BigInt): number | null;
  offset_at(segment: number): number | null;
  segment_range(segment: number): {start: number, end: number} | null;
  // segment_at_pos(bp: BigInt): Promise<number | null>;
  // offset_at(segment: number): Promise<number | null>;
  // segment_range(segment: number): Promise<{start: number, end: number} | null>;

}


export class CoordSysArrow {
  table: Table;

  constructor(table: Table) {
    this.table = table;
  }

  max_f64(): number {
    let offsets = this.table.getChild('step_offsets')!;
    return offsets.get(offsets.length - 1);
  }

  segmentAtPosition(bp: BigInt): number | null {
    // let pos_i = binarySearch(this.table.getChild('step_offsets')!, Number(bp), (a, b) => a - b);
    // TODO
    return null;
  }

  segmentOffset(segment: number): number | null {
    // TODO
    return null;
  }

  segmentRange(segment: number): number | null {
    // TODO
    return null;
  }

}


export async function coordSysFromTable(
  table: Table,
){
  return new CoordSysArrow(table);
}
