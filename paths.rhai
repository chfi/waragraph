export let out_image = image_var("out_image");
export let out_image_view = image_view_var("out_image_view");


export let font_image = allocate_image("font_image",
                                       1024,
                                       8,
                                       vk::Format::R8G8B8A8_UNORM,
                                       vk::ImageUsageFlags::STORAGE
                                       | vk::ImageUsageFlags::SAMPLED
                                       | vk::ImageUsageFlags::TRANSFER_SRC
                                       | vk::ImageUsageFlags::TRANSFER_DST);

export let font_image_view = image_view_for(font_image);


export let path_buffer_0 = buffer_var("path_0");
// export let path_buffer_1 = buffer_var("path_1");
// export let path_buffer_2 = buffer_var("path_2");
// export let path_buffer_3 = buffer_var("path_3");
// export let path_buffer_4 = buffer_var("path_4");

export let clip_rects_buffer =
  allocate_buffer("rect_buffer",
                  vk::MemoryLocation::CPU_TO_GPU,
                  1,
                  4 * 4 * 8, // vec4 * 8
                  vk::BufferUsageFlags::TRANSFER_DST
                  | vk::BufferUsageFlags::STORAGE_BUFFER);


let path_binds = [vk::binding::storage_buffer(0),
                  vk::binding::storage_buffer(1),
                  vk::binding::storage_image(2)];

let path_set_infos = read_shader_descriptors("shaders/path.comp.spv");

export let path_desc_set_0 =
  create_desc_set(vk::ShaderStageFlags::COMPUTE,
                  path_set_infos,
                  0,
                  [#{ binding: 0, buffer: path_buffer_0 },
                   #{ binding: 1, buffer: clip_rects_buffer },
                   #{ binding: 2, image_view: out_image_view }]);

export let path_pipeline =
  load_compute_shader("shaders/path.comp.spv", path_binds, 12);


fn init() {
  let init_batch = batch_builder();

  init_batch.load_image_from_file("8x8font.png",
                                  self::font_image.get(),
                                  vk::ImageLayout::GENERAL);

  let clip_buf = self::clip_rects_buffer.get();
  let clip_data = blob();

  for i in 0..5 {
    let fi = i.to_float();

    let x = 20.0;
    let y = 20.0 + 56.0 * fi;

    let w = 760.0;
    let h = 50.0;

    clip_data.append_float(x);
    clip_data.append_float(y);
    clip_data.append_float(w);
    clip_data.append_float(h);

  }

  init_batch.initialize_buffer_with(clip_buf, clip_data);
  // init_batch.initialize_buffer_with(Fn("rect_blob"), rect_buf, rects);

  init_batch
}

// fn create_path_buffer(path_name, node_count) {

// }


fn foreground(w, h, node_count) {
  let batch = batch_builder();


  let img = self::out_image.get();
  batch.transition_image(img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         // vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                         vk::ImageLayout::UNDEFINED,
                         vk::ImageLayout::GENERAL);

  let pc = blob();
  pc.append_int(w);
  pc.append_int(h);
  pc.append_int(node_count);

  let x_size = 8;
  let y_size = 8;

  let x_groups = (w / x_size) + w % x_size;
  let y_groups = (h / y_size) + h % y_size;
  let z_groups = 1;


  batch.dispatch_compute(self::path_pipeline.get(),
                         self::path_desc_set_0.get(),
                         pc,
                         x_groups,
                         y_groups,
                         z_groups);

  batch.transition_image(img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::AccessFlags::SHADER_READ,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::ImageLayout::GENERAL,
                         vk::ImageLayout::TRANSFER_SRC_OPTIMAL);

  batch
}
