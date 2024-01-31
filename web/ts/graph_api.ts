import { Bp, PathId } from "./types";




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

  // depth data; or put that in DatasetStore & have the worker do it
}


export interface PathIndex {
  pathsOnSegment(segment: number): Promise<Uint32Array | undefined>;
  // whole matrix too? don't even have a sparse matrix library for JS pulled in yet
}


export interface GraphLayout {
  sample2DPath(path_id: PathId, start: Bp, end: Bp, tolerance: number): Promise<Float32Array | undefined>;

  segmentPosition(segment: number): Promise<Float32Array | undefined>;
}


export async function serverAPIs(base_url: URL): Promise<
  { arrowGFA: ArrowGFA, pathIndex: PathIndex, graphLayout: GraphLayout }
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
      const resp = await fetch(new URL(`/coordinate_system/segment_at_path_position?path_id=${path}&pos_bp=${pos}`, base_url));
      const json = await resp.json();
      return json;
    }
  };

  const pathIndex = {
    async pathsOnSegment(segment: number): Promise<Uint32Array | undefined> {
      console.warn("TODO implement server path index");
      return undefined;
    }
  };

  const graphLayout = {
    async sample2DPath(path: PathId, start: Bp, end: Bp, tolerance: number): Promise<Float32Array | undefined> {
      const query =
        new URL(`/graph_layout/sample_path_id?path_id=${path}&start_bp=${start}&end_bp=${end}&tolerance=${tolerance}`, base_url);
      const resp = await fetch(query);
      const buffer = await resp.arrayBuffer();
      return new Float32Array(buffer);
    },

    async segmentPosition(segment: number): Promise<Float32Array | undefined> {
      const resp = await fetch(new URL(`/graph_layout/segment_position/${segment}`, base_url));
      if (resp.ok === false) {
        return;
      }

      const buf = await resp.arrayBuffer();
      return new Float32Array(buf);
    }
  };


  return { arrowGFA, pathIndex, graphLayout };
}
