

// const comlink_url = "https://unpkg.com/comlink/dist/umd/comlink.js";


// importScripts();
// importScripts("../../../dist/umd/comlink.js");


// function getWorkerURL(url) {
//     const content = `importScripts("${url}");`;
//     return URL.createObjectURL(new Blob([content], { type: "text/javascript" }));
// }

          // console.log("eh");


// import * as wasm from './pkg/web.js';
// import * as Comlink from "https://unpkg.com/comlink/dist/esm/comlink.mjs";
// import * as Comlink from "./comlink.mjs";

          // console.log("hm");

// console.log(typeof wasm);
// console.log(wasm());
// console.log(wasm.wasm_bindgen);


// console.log("what: " + wasm);
// console.log(wasm_bindgen);

          // console.log("??????");

importScripts('./pkg/web.js');
// importScripts(comlink_url);
importScripts("./comlink.js");


console.log(wasm_bindgen);
console.log(typeof wasm_bindgen);

wasm_bindgen('./pkg/web_bg.wasm')
    .then((w) => {
        console.log("done???");
        console.log(w);
    });

// for (const [key, value] of Object.entries(wasm_bindgen)) {
//   console.log(`${key}: ${value}`);
// }

/*
async function init_context() {

    const obj = {
        counter: 0,
        inc() {
            this.counter++;
        },
    };

    Comlink.expose(obj);

}


init_context();
*/
