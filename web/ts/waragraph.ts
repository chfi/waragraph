import init_module, * as wasm_bindgen from 'waragraph';

import type { WorkerCtxInterface } from './main_worker';
import type { Bp, Segment, Handle, PathIndex } from './types';

import { BehaviorSubject } from 'rxjs';

import { mat3, vec2 } from 'gl-matrix';


// there really should be a better name for this...
interface CoordinateSystem {
  coord_sys: wasm_bindgen.CoordSys;
}

interface View2D {
  center: vec2;
  size: vec2;
}

class Viewport2D {
  // not sure if it makes sense to store the segment positions as is in the viewport, even
  // though they are associated with it in a similar way to the 1D coordinate systems;
  // SegmentPositions is currently using owned Vecs, but here on the JS side we could share
  // however many pointers we want to it (it's immutable)
  segment_positions: wasm_bindgen.SegmentPositions;
  view: wasm_bindgen.View2D;
  subject: BehaviorSubject<View2D>; // doesn't make sense to use wasm pointers here

  constructor() {
  }
  
}

interface View1D {
  start: Bp;
  end: Bp;
}

// one instance of this would be shared by all 1D views that should be "synced up"
// including external tracks, eventually
class Viewport1D {
  coord_sys: wasm_bindgen.CoordSys;
  view: wasm_bindgen.View1D;
  subject: BehaviorSubject<View1D>;

  constructor(coord_sys: wasm_bindgen.CoordSys, view: wasm_bindgen.View1D) {
    this.coord_sys = coord_sys;
    this.view = view;

    let view_range = { start: view.start, end: view.end };

    this.subject = new BehaviorSubject(view_range as View1D);
  }

  segmentAtOffset(bp: Bp) {
    if (typeof bp === "number") {
      bp = BigInt(bp);
    }
    return this.coord_sys.segment_at_pos(bp);
  }

  segmentOffset(segment: Segment) {
    return this.coord_sys.offset_at(segment);
  }

  segmentRange(segment: Segment): { start: Bp, end: Bp } {
    return this.coord_sys.segment_range(segment) as { start: Bp, end: Bp };
  }

}

export class Waragraph {
  /*
  needs to hold coordinate systems, viewports, and data,
  in addition to the graph

  it should probably also have a reference to the wasm module & memory

  upon initialization, assuming only the graph is given (for now the graph
  will be mandatory and restricted to just one), all that should happen is:
   * worker pool initialized
   * graph parsed and loaded on worker
   * computable datasets table is set up with defaults


  the worker pool is probably a bit much to implement right now (i'm not 100%
  sure what the boundary looks like), so instead clean up and refine the worker module
  
  

   */

    constructor() {
    }

  // this would be responsible for "storing" the coordinate systems
  // (still just pointers here), but they should still be
  // computed/created by a worker

  
}



// export async function initializeWaragraph({ } = {}): Waragraph {
export async function initializeWaragraph({ } = {}) {



  //

}
