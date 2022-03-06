export let out_image = image_var("out_image");
export let out_image_view = image_view_var("out_image_view");
export let out_desc_set = desc_set_var("out_desc_set");

export let color_buf = buffer_var("color_buffer");

export let bg_shader = load_shader("shaders/bg_color.comp.spv",
                                   vk::ShaderStageFlags::COMPUTE);

export let bg_pipeline = create_compute_pipeline(bg_shader);

// export let bg_desc_set =
//   create_desc_set(bg_shader,
//                   0,
//                   [ #{ binding: 0, image_view: out_image_view }]);

export let font_image = allocate_image("font_image",
                                       1024,
                                       8,
                                       vk::Format::R8G8B8A8_UNORM,
                                       vk::ImageUsageFlags::STORAGE
                                       | vk::ImageUsageFlags::SAMPLED
                                       | vk::ImageUsageFlags::TRANSFER_SRC
                                       | vk::ImageUsageFlags::TRANSFER_DST);

export let font_image_view = image_view_for(font_image);


export let clip_rects_buffer =
  allocate_buffer("rect_buffer",
                  vk::MemoryLocation::CPU_TO_GPU,
                  1,
                  4 * 4 * 8, // vec4 * 8
                  vk::BufferUsageFlags::TRANSFER_DST
                  | vk::BufferUsageFlags::STORAGE_BUFFER);


export let path_shader = load_shader("shaders/path_new.comp.spv",
                                     vk::ShaderStageFlags::COMPUTE);


export let color_desc_set =
  create_desc_set(path_shader,
                  2,
                  [ #{ binding: 0, buffer: color_buf } ]);

export let path_pipeline = create_compute_pipeline(path_shader);

fn rect_blob(rect) {
  let data = blob();
  data.append_float(rect.x);
  data.append_float(rect.y);
  data.append_float(rect.w);
  data.append_float(rect.h);
  data
}

fn resize(width, height) {
  let init_batch = batch_builder();

  let clip_buf = self::clip_rects_buffer.get();
  let clip_rects = [];

  let w = width.to_float() - 40.0;
  let h = 50.0;

  for i in 0..5 {
    clip_rects.push(#{ x: 20.0,  y: 20.0 + 56.0 * i,
                       w: w, h: h });
  }

  init_batch.initialize_buffer_with(Fn("rect_blob"),
                                    self::clip_rects_buffer.get(),
                                    clip_rects);

  init_batch
}

fn init() {
  let init_batch = batch_builder();

  init_batch.load_image_from_file("8x8font.png",
                                  self::font_image.get(),
                                  vk::ImageLayout::GENERAL);

  let clip_buf = self::clip_rects_buffer.get();
  let clip_rects = [];

  for i in 0..5 {
    clip_rects.push(#{ x: 20.0,  y: 20.0 + 56.0 * i,
                       w: 760.0, h: 50.0 });
  }

  init_batch.initialize_buffer_with(Fn("rect_blob"),
                                    self::clip_rects_buffer.get(),
                                    clip_rects);

  init_batch
}

fn foreground(batch, desc_sets, w, h, node_count) {

  let img = self::out_image.get();
  batch.transition_image(img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::ImageLayout::UNDEFINED,
                         vk::ImageLayout::GENERAL);

  // draw background
  let pc = blob();
  // background color in push constant
  pc.append_float(0.95);
  pc.append_float(0.95);
  pc.append_float(0.95);
  pc.append_float(1.0);
  pc.append_int(w);
  pc.append_int(h);

  let x_size = 16;
  let y_size = 16;

  let x_groups = (w / x_size) + w % x_size;
  let y_groups = (h / y_size) + h % y_size;

  batch.dispatch_compute(self::bg_pipeline.get(),
                         self::out_desc_set.get(),
                         // self::bg_desc_set.get(),
                         pc, x_groups, y_groups, 1);


  // draw paths

  let x_size = 8;
  let y_size = 8;

  let workgroups = #{ x_groups: (w / x_size) + w % x_size,
                      y_groups: (h / y_size) + h % y_size,
                      z_groups: 1 };

  for ix in 0..desc_sets.len() {

    let desc_sets = desc_sets[ix];

    let slot_set = desc_sets.slot;
    let clip_set = desc_sets.clip;

    let pc = blob();
    pc.append_int(w);
    pc.append_int(h);
    pc.append_int(node_count);
    pc.append_int(ix);

    batch.dispatch_compute(self::path_pipeline.get(),
                           [slot_set,
                            clip_set,
                            self::color_desc_set.get(),
                            self::out_desc_set.get()],
                           pc,
                           workgroups);
  }

  batch.transition_image(img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::AccessFlags::SHADER_READ,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::ImageLayout::GENERAL,
                         vk::ImageLayout::TRANSFER_SRC_OPTIMAL);

  batch
}
