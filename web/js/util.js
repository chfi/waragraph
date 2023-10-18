export function create_image_data_impl(mem, data_ptr, data_len) {
    const mem_src = new Uint8ClampedArray(mem.buffer, data_ptr, data_len);

    const buf_dst = new ArrayBuffer(data_len);
    const dst_view = new Uint8ClampedArray(buf_dst);
    dst_view.set(mem_src);

    const img_data = new ImageData(dst_view, data_len / 4);
    return img_data;
}

export function create_mat3_impl(mem, data_ptr) {
    const data_len = 4 * 9;
    const mem_src = new Float32Array(mem.buffer, data_ptr, data_len);

    const buf_dst = new ArrayBuffer(data_len);
    const dst_view = new Float32Array(buf_dst);
    dst_view.set(mem_src);

    return dst_view;
}
