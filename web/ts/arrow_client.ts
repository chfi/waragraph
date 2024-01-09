import { tableFromIPC } from 'apache-arrow';


export async function readArrowGraphLayout(src: URL) {
  console.log("fetching data");
  let data = await fetch(src);

  console.log("data fetched; deserializing into table");
  let table = await tableFromIPC(data);
  console.log(table);

  return table;
}
