

import { Table } from 'apache-arrow';


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
