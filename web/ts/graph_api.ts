



export interface CoordSysStore {

}

export interface DatasetStore {

}


export type PathMetadata = {
  id: number,
  name: string,
  stepCount: number,
  lengthBp: BigInt,
};

export interface ArrowGFA {
  segmentSequencesArray(): Promise<Uint8Array>;
  pathIdFromName(name: string): Promise<number | null>;
  pathNameFromId(id: number): Promise<string | null>;
  pathMetadata(): Promise<[PathMetadata]>;
  pathSteps(id: number): Promise<Uint32Array | null>;

  // depth data; or put that in DatasetStore & have the worker do it
}


export interface PathIndex {
  pathsOnSegment(segment: number): Promise<Uint32Array | null>;
  // whole matrix too? don't even have a sparse matrix library for JS pulled in yet
}
