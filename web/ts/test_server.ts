
import { initializePathViewerClient } from './path_viewer_ui';
import { Viewport1D } from './waragraph';



export async function testPathViewer(base_url: URL) {


  let paths_resp = await fetch(new URL('/path_metadata', base_url));
  let paths = await paths_resp.json();

  // console.log(paths);

  // const 

  // const viewport = new Viewport1D(

  for (const path of paths) {
    // const viewer = initializePathViewerClient(path.name
    console.log(path);
  }

}
