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
  view: wasm_bindgen.View1D;
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

  

   */

  
}



// export async function initializeWaragraph({ } = {}): Waragraph {
export async function initializeWaragraph({ } = {}) {



  //

}
