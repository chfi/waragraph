// NOTE: This file creates a service worker that cross-origin-isolates the page (read more here: https://web.dev/coop-coep/) which allows us to use wasm threads.
// Normally you would set the COOP and COEP headers on the server to do this, but Github Pages doesn't allow this, so this is a hack to do that.
/* Edited version of: coi-serviceworker v0.1.6 - Guido Zuidhof, licensed under MIT */// From here: https://github.com/gzuidhof/coi-serviceworker
if("undefined"==typeof window){async function e(e){if("only-if-cached"===e.cache&&"same-origin"!==e.mode)return;"no-cors"===e.mode&&(e=new Request(e.url,{cache:e.cache,credentials:"omit",headers:e.headers,integrity:e.integrity,destination:e.destination,keepalive:e.keepalive,method:e.method,mode:e.mode,redirect:e.redirect,referrer:e.referrer,referrerPolicy:e.referrerPolicy,signal:e.signal}));let r=await fetch(e).catch(e=>console.error(e));if(0===r.status)return r;let t=new Headers(r.headers);return t.set("Cross-Origin-Embedder-Policy","credentialless"),t.set("Cross-Origin-Opener-Policy","same-origin"),new Response(r.body,{status:r.status,statusText:r.statusText,headers:t})}self.addEventListener("install",()=>self.skipWaiting()),self.addEventListener("activate",e=>e.waitUntil(self.clients.claim())),self.addEventListener("fetch",function(r){r.respondWith(e(r.request));// respondWith must be executed synchonously (but can be passed a Promise)
})}else!async function(){if(!1!==window.crossOriginIsolated)return;let e=await navigator.serviceWorker.register(window.document.currentScript.src).catch(e=>console.error("COOP/COEP Service Worker failed to register:",e));e&&(console.log("COOP/COEP Service Worker registered",e.scope),e.addEventListener("updatefound",()=>{console.log("Reloading page to make use of updated COOP/COEP Service Worker."),window.location.reload()}),e.active&&!navigator.serviceWorker.controller&&(console.log("Reloading page to make use of COOP/COEP Service Worker."),window.location.reload()))}();// Code to deregister:
// let registrations = await navigator.serviceWorker.getRegistrations();
// for(let registration of registrations) {
//   await registration.unregister();
// }
//# sourceMappingURL=index.8c5732ef.js.map

//# sourceMappingURL=index.8c5732ef.js.map
