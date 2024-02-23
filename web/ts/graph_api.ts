import * as wasm_bindgen from 'waragraph';

import * as Comlink from 'comlink';

import { Bp, PathId } from "./types";
import { WaragraphWorkerCtx } from './worker';




export interface CoordSysStore {

}

export interface DatasetStore {

}


export type PathMetadata = {
  id: number,
  name: string,
  stepCount: number,
  // lengthBp: BigInt,
};

export interface ArrowGFA {
  segmentSequencesArray(): Promise<Uint8Array>;
  pathIdFromName(name: string): Promise<number | undefined>;
  pathNameFromId(id: number): Promise<string | undefined>;
  pathMetadata(): Promise<[PathMetadata]>;
  pathSteps(id: number): Promise<Uint32Array | undefined>;
  segmentAtPathPosition(path: PathId, pos: Bp): Promise<number | undefined>;
  segmentAtGlobalPosition(pos: Bp): Promise<number | undefined>;
  segmentGlobalRange(segment: number): Promise<{ start: bigint, end: bigint } | undefined>;
  // depth data; or put that in DatasetStore & have the worker do it

}


export interface PathIndex {
  pathsOnSegment(segment: number): Promise<Uint32Array | undefined>;
  // whole matrix too? don't even have a sparse matrix library for JS pulled in yet
}


// export interface GraphLayout {
//   sample2DPath(path_id: PathId, start: Bp, end: Bp, tolerance: number): Promise<Float32Array | undefined>;
//   segmentPosition(segment: number): Promise<Float32Array | undefined>;
// }


export async function standaloneAPIs(worker: Comlink.Remote<WaragraphWorkerCtx>): Promise<
  { arrowGFA: ArrowGFA, pathIndex: PathIndex }
> {

  const arrowGFA = {
    async segmentSequencesArray(): Promise<Uint8Array> {
      return worker.segmentSequencesArray();
    },

    async pathIdFromName(name: string): Promise<number | undefined> {
      return worker.pathIdFromName(name);
    },

    async pathNameFromId(id: number): Promise<string | undefined> {
      return worker.pathNameFromId(id);
    },

    async pathMetadata(): Promise<[PathMetadata]> {
      return worker.pathMetadata();
    },

    async pathSteps(id: number): Promise<Uint32Array | undefined> {
      return worker.pathSteps(id);
    },

    async segmentAtPathPosition(path: PathId, pos: Bp): Promise<number | undefined> {
      return worker.segmentAtPathPosition(path, pos);
    },

    async segmentAtGlobalPosition(pos: Bp): Promise<number | undefined> {
      return worker.segmentAtGlobalPosition(pos);
    },

    async segmentGlobalRange(segment: number): Promise<{ start: bigint, end: bigint } | undefined> {
      return worker.segmentGlobalRange(segment);
    }
  };

  const pathIndex = {
    async pathsOnSegment(segment: number): Promise<Uint32Array | undefined> {
      return worker.pathsOnSegment(segment);
    }
  };
  
  return { arrowGFA, pathIndex };
}


export async function serverAPIs(base_url: URL): Promise<
  { arrowGFA: ArrowGFA, pathIndex: PathIndex }
> {

  let segmentSequences: Uint8Array | undefined = undefined;

  let path_metadata = await fetch(new URL('/path_metadata', base_url)).then(resp => resp.json());

  let path_name_id_map = new Map();
  path_metadata.forEach(({ name, id }) => {
    path_name_id_map.set(name, id);
  });


  const arrowGFA = {
    async segmentSequencesArray(): Promise<Uint8Array> {
      if (segmentSequences !== undefined) {
        return segmentSequences;
      }

      const resp = await fetch(new URL("/sequence_array", base_url));
      const buffer = await resp.arrayBuffer();
      return new Uint8Array(buffer);
    },

    async pathIdFromName(name: string): Promise<number | undefined> {
      return path_name_id_map.get(name);
    },

    async pathNameFromId(id: number): Promise<string | undefined> {
      return path_metadata[id]?.name;
    },

    async pathMetadata(): Promise<[PathMetadata]> {
      return path_metadata;
    },

    async pathSteps(id: number): Promise<Uint32Array | undefined> {
      const resp = await fetch(new URL(`/path_steps/${id}`, base_url));
      const buffer = await resp.arrayBuffer();
      return new Uint32Array(buffer);
    },

    async segmentAtPathPosition(path: PathId, pos: Bp): Promise<number | undefined> {
      const resp = await fetch(new URL(`/coordinate_system/path/segment_at_offset?path_id=${path}&pos_bp=${pos}`, base_url));
      const json = await resp.json();
      return json;
    },

    async segmentAtGlobalPosition(pos: Bp): Promise<number | undefined> {
      const resp = await fetch(new URL(`/coordinate_system/global/segment_at_offset?pos_bp=${pos}`, base_url));
      const json = await resp.json();
      return json;
    },

    async segmentGlobalRange(segment: number): Promise<{ start: bigint, end: bigint } | undefined> {
      const resp = await fetch(new URL(`/coordinate_system/global/segment_range/${segment}`, base_url));
      if (!resp.ok) {
        return;
      }
      const buf = await resp.arrayBuffer();
      const array = new BigUint64Array(buf);
      if (array.length != 2) {
        return;
      }
      const start = array.at(0)!;
      const end = array.at(1)!;
      return { start, end };
    }
   

  };

  const pathIndex = {
    async pathsOnSegment(segment: number): Promise<Uint32Array | undefined> {
      const resp = await fetch(new URL(`/paths_on_segment/${segment}`, base_url));
      if (!resp.ok) {
        return;
      }
      const resp_buf = await resp.arrayBuffer();
      return new Uint32Array(resp_buf);
    }
  };


  return { arrowGFA, pathIndex };
}
