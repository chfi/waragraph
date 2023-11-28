

/* when Rust objects are passed across a worker boundary and
(de)serialized, we only have the pointer, since the class instance is
"attached" to a module instance

wasm_bindgen's JS includes a static `__wrap` method that takes a
pointer and wraps it in the class, but it's not exposed in the
typescript declarations, hence this reimplementation
 */
export function wrapWasmPtr(class_, ptr) {
  ptr = ptr >>> 0;
  const obj = Object.create(class_.prototype);
  obj.__wbg_ptr = ptr;
  return obj;
}
