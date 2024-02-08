import { vec2 } from "gl-matrix";
import { GraphLayoutTable } from "./graph_layout";
import { GraphViewer, View2DObj } from "./graph_viewer";

export interface SVGExportOptions {
  min_world_length?: number,
}

export function export2DViewportSvg(
  graph_viewer: GraphViewer,
  graph_layout: GraphLayoutTable,
  color: (segment: number) => { r: number, g: number, b: number, a: number },
  options?: SVGExportOptions,
): SVGSVGElement {

  const view = graph_viewer.getView();
  // const mat = graph_viewer.getViewMatrix();

  const aabbMin = vec2.fromValues(view.x - view.width / 2.0, view.y - view.height / 2.0);
  const aabbMax = vec2.fromValues(view.x + view.width / 2.0, view.y + view.height / 2.0);

  const svg_el = document.createElementNS('http://www.w3.org/2000/svg', 'svg') as SVGSVGElement;

  svg_el.setAttribute('transform', 'matrix(1 0 0 -1 0 0)');
  svg_el.setAttribute('viewBox', `${aabbMin[0]} ${aabbMin[1]} ${view.width} ${view.height}`);

  const iter = graph_layout.iterateSegments();

  const min_world_length = options?.min_world_length ? options.min_world_length : 0.0;

  // need to iterate in pairs, as when filling the buffers
  for (const { segment, p0, p1 } of iter) {

    if (lineSegmentIntersectsAABB2D(aabbMin, aabbMax, p0, p1)) {

      const len = vec2.distance(p0, p1);

      if (len < min_world_length) {
        continue;
      }

      const el = document.createElementNS('http://www.w3.org/2000/svg', 'line') as SVGLineElement;
      el.setAttribute('x1', String(p0[0]));
      el.setAttribute('y1', String(p0[1]));
      el.setAttribute('x2', String(p1[0]));
      el.setAttribute('y2', String(p1[1]));

      // TODO
      const {r,g,b,a} = color(segment);
      if (a === 0) {
        continue;
      }

      const fmt = (v: number) => Math.round(v * 255);
      el.setAttribute('stroke', `rgb(${fmt(r)} ${fmt(g)} ${fmt(b)}`);
      el.setAttribute('stroke-opacity', `${a}`);
      el.setAttribute('stroke-width', '1%');

      svg_el.append(el);
    }
  }

  return svg_el;
}

function lineSegmentIntersectsAABB2D(aabbMin: vec2, aabbMax: vec2, p1: vec2, p2: vec2): boolean {
  // Calculate deltas
  const dx = p2[0] - p1[0];
  const dy = p2[1] - p1[1];
  
  // Calculate the min and max t for x and y axes
  let tmin = -Infinity;
  let tmax = Infinity;

  if (dx === 0 && (p1[0] < aabbMin[0] || p1[0] > aabbMax[0])) {
    // Line is parallel to Y-axis and outside AABB
    return false;
  } else if (dx !== 0) {
    // Compute intersection t value of ray with near and far vertical edges of AABB
    let tx1 = (aabbMin[0] - p1[0]) / dx;
    let tx2 = (aabbMax[0] - p1[0]) / dx;

    tmin = Math.max(tmin, Math.min(tx1, tx2));
    tmax = Math.min(tmax, Math.max(tx1, tx2));
  }

  if (dy === 0 && (p1[1] < aabbMin[1] || p1[1] > aabbMax[1])) {
    // Line is parallel to X-axis and outside AABB
    return false;
  } else if (dy !== 0) {
    // Compute intersection t value of ray with near and far horizontal edges of AABB
    let ty1 = (aabbMin[1] - p1[1]) / dy;
    let ty2 = (aabbMax[1] - p1[1]) / dy;

    tmin = Math.max(tmin, Math.min(ty1, ty2));
    tmax = Math.min(tmax, Math.max(ty1, ty2));
  }

  // If there are any intersections along the line segment, they must occur between t = 0 and t = 1
  return tmax >= tmin && tmin <= 1 && tmax >= 0;
}

