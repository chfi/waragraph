import { Table } from "apache-arrow";
import { vec2 } from "gl-matrix";



export class GraphLayout {
  layout_table: Table;

  segmentPosition(segment: number): { p0: vec2, p1: vec2 } | null {
    if (!Number.isInteger(segment)) {
      return null;
    }

    let s0 = segment << 1;
    let s1 = s0 + 1;

    let q0 = this.layout_table.get(s0);
    let q1 = this.layout_table.get(s1);

    if (q0 === null || q1 === null) {
      return null;
    }

    let p0 = vec2.fromValues(q0['x'], q0['y']);
    let p1 = vec2.fromValues(q1['x'], q1['y']);

    return { p0, p1 };
  }
}
