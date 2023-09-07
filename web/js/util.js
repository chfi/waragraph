
export function create_image_data_impl(mem, data_ptr, data_len) {
    console.log("1111");
    const buf_dst = new ArrayBuffer(data_len);
    console.log("2222");
    const mem_src = new Uint8ClampedArray(mem);
    console.log("3333");
    const mem_sliced = mem_src.slice(data_ptr, data_len);
    console.log("4444");

    const dst_view = new Uint8ClampedArray(buf_dst);

    dst_view.set(mem_sliced);

    console.log("" + mem_sliced);
    console.log("" + dst_view);
    console.log("" + buf_dst);
    console.log("5555");
    const img_data = new ImageData(buf_dst, data_len / 4);
    console.log("6666");
    // let view = new Uint8Array(data_ptr, data_len);

    return img_data;
}


export function madness(aaaa) {
    console.log("AAAAAAAAAAAAA");
    console.log(aaaa);
    return aaaa;
}
