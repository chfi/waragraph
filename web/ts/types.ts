

export type Bp = number | bigint;

export type Segment = number;
export type Handle = number;
export type PathId = number;

export type PathInterval =
    { path_id: PathId, start: Bp, end: Bp }
  | { path_name: string, start: Bp, end: Bp };


// idk about this but w/e
export interface RGBObj {
  r: number,
  g: number,
  b: number,
}

export interface RGBAObj {
  r: number,
  g: number,
  b: number,
  a: number,
}
