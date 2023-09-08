export function create_image_data_impl(mem, data_ptr, data_len) {
    const mem_src = new Uint8ClampedArray(mem.buffer, data_ptr, data_len);

    const buf_dst = new ArrayBuffer(data_len);
    const dst_view = new Uint8ClampedArray(buf_dst);
    dst_view.set(mem_src);

    const img_data = new ImageData(dst_view, data_len / 4);
    return img_data;
}
