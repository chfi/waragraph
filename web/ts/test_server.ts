
import init_module, * as wasm_bindgen from 'waragraph';

import { initializePathViewerClient } from './path_viewer_ui';
import { Viewport1D } from './waragraph';

import { tableFromIPC, tableFromArrays } from 'apache-arrow';
import { CoordSysArrow } from './coordinate_system';


export async function testPathViewer(base_url: URL) {
  const wasm = await init_module();

  let paths_resp = await fetch(new URL('/path_metadata', base_url));
  let paths = await paths_resp.json();

  
  let cs_resp = await fetch(new URL('/coordinate_system/global', base_url));

  if (!cs_resp.ok) {
    return;
  }
  let cs = await tableFromIPC(cs_resp);

  let step_offsets = cs.getChild('step_offsets')!;
  let max = step_offsets.get(step_offsets.length - 1);

  let cs_arrow = new CoordSysArrow(cs);

  console.log(cs);

  console.log("???");

  // console.log(paths);

  // const 

  // const viewport = new Viewport1D(

  for (const path of paths) {
    // const viewer = initializePathViewerClient(path.name
    console.log(path);
  }

}
