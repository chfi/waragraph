export let out_image = image_var("out_image");
export let out_image_view = image_view_var("out_image_view");
export let out_desc_set = desc_set_var("out_desc_set");

let unnorm_sampler_info =
#{ mag_filter: vk::Filter::NEAREST,
   min_filter: vk::Filter::NEAREST,
   address_mode_u: vk::SamplerAddressMode::REPEAT,
   address_mode_v: vk::SamplerAddressMode::REPEAT,
   address_mode_w: vk::SamplerAddressMode::REPEAT,
   anisotropy_enable: false,
   unnormalized_coordinates: false,
   mipmap_mode: vk::SamplerMipmapMode::NEAREST,
   mip_lod_bias: 0.0,
   min_lod: 0.0,
   max_lod: 1.0,
 };

let norm_sampler_info =
#{ mag_filter: vk::Filter::NEAREST,
   min_filter: vk::Filter::NEAREST,
   address_mode_u: vk::SamplerAddressMode::REPEAT,
   address_mode_v: vk::SamplerAddressMode::REPEAT,
   address_mode_w: vk::SamplerAddressMode::REPEAT,
   anisotropy_enable: false,
   unnormalized_coordinates: false,
   mipmap_mode: vk::SamplerMipmapMode::NEAREST,
   mip_lod_bias: 0.0,
   min_lod: 0.0,
   max_lod: 1.0,
 };

export let unnorm_sampler = create_sampler(unnorm_sampler_info);
export let norm_sampler = create_sampler(norm_sampler_info);

export let bg_shader = load_shader("shaders/bg_color.comp.spv",
                                   vk::ShaderStageFlags::COMPUTE);

export let bg_pipeline = create_compute_pipeline(bg_shader);

// text

export let text_shader = load_shader("shaders/text.comp.spv",
                                   vk::ShaderStageFlags::COMPUTE);


export let text_pipeline = create_compute_pipeline(text_shader);

// line rasterizer
export let line_shader = load_shader("shaders/lines_sdf.comp.spv",
                                     vk::ShaderStageFlags::COMPUTE);
export let line_pipeline = create_compute_pipeline(line_shader);


// font

export let font_image = allocate_image("font_image",
                                       1024,
                                       8,
                                       vk::Format::R8G8B8A8_UNORM,
                                       vk::ImageUsageFlags::STORAGE
                                       | vk::ImageUsageFlags::SAMPLED
                                       | vk::ImageUsageFlags::TRANSFER_SRC
                                       | vk::ImageUsageFlags::TRANSFER_DST);

export let font_image_view = image_view_for(font_image);


export let font_desc_set =
  create_desc_set(text_shader,
                  0,
                  [ #{ binding: 0, image_view: font_image_view } ,
                    #{ binding: 1, sampler: norm_sampler } ]);


export let path_shader = load_shader("shaders/path_slot_indexed.comp.spv",
                                     vk::ShaderStageFlags::COMPUTE);


export let path_pipeline = create_compute_pipeline(path_shader);

export let rect = #{ x: 20.0, y: 48.0, w: -40.0, h: 40.0 };

fn resize(width, height) {
  let init_batch = batch_builder();

  let img = self::out_image.get();

  init_batch.transition_image(self::out_image.get(),
                              vk::AccessFlags::empty(),
                              vk::PipelineStageFlags::TOP_OF_PIPE,
                              vk::AccessFlags::SHADER_READ
                              | vk::AccessFlags::SHADER_WRITE,
                              vk::PipelineStageFlags::COMPUTE_SHADER,
                              vk::ImageLayout::UNDEFINED,
                              vk::ImageLayout::GENERAL);

  init_batch
}

fn init() {
  let init_batch = batch_builder();

  init_batch.load_image_from_file("8x8font.png",
                                  self::font_image.get(),
                                  vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

  let img = self::out_image.get();

  init_batch.transition_image(self::out_image.get(),
                              vk::AccessFlags::empty(),
                              vk::PipelineStageFlags::TOP_OF_PIPE,
                              vk::AccessFlags::SHADER_READ
                              | vk::AccessFlags::SHADER_WRITE,
                              vk::PipelineStageFlags::COMPUTE_SHADER,
                              vk::ImageLayout::UNDEFINED,
                              vk::ImageLayout::GENERAL);

  init_batch
}

fn foreground(batch, desc_sets, slot_w, w, h) {

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
  let dims = #{ width: w, height: h };
  let batch = draw_slots(batch,
                         dims, #{ width: slot_w, height: 50 },
                         desc_sets,
                         "gradient_rainbow");
  // let batch = draw_slots(batch, h, #{ width: slot_w, height: 50 }, desc_sets, "gradient_rainbow");

  batch.transition_image(img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::AccessFlags::SHADER_READ,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::ImageLayout::GENERAL,
                         vk::ImageLayout::GENERAL);


  /*
  // draw lines

  // let line_data_set = buffers::get_uvec4("line_storage").storage_set;

  // let line_data = buffers::get_uvec4("line_storage");
  let line_data = get_buffer("line_storage");
  let len = line_data.len; // should be same as GPUs line_count

  let pc = blob();

  pc.append_float(0.0);
  pc.append_float(0.0);
  pc.append_float(0.0);
  pc.append_float(1.0);

  pc.append_int(w);
  pc.append_int(h);

  pc.append_float(1.0);

  // let x_size = 64;
  let x_size = 8;
  let y_size = 8;

  let workgroups = #{ x_groups: (w / x_size) + w % x_size,
                      y_groups: (h / y_size) + h % y_size,
                      z_groups: 1 };


  batch.dispatch_compute(self::line_pipeline.get(),
                         [line_data.set,
                          self::out_desc_set.get()],
                         pc,
                         workgroups);
                         */


  // complete

  batch.transition_image(img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::AccessFlags::SHADER_READ,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::ImageLayout::GENERAL,
                         vk::ImageLayout::GENERAL);
                         // vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                         // vk::ImageLayout::TRANSFER_SRC_OPTIMAL);

  batch
}



export let img_shader = load_shader("shaders/copy_image.comp.spv",
                                    vk::ShaderStageFlags::COMPUTE);

export let img_pipeline = create_compute_pipeline(img_shader);



// `swapchain` should be #{ img, img_view, storage_set } for the correct swapchain image
fn copy_to_swapchain(batch, source_set, swapchain, w, h) {

  batch.transition_image(swapchain.img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::AccessFlags::SHADER_READ,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::ImageLayout::UNDEFINED,
                         vk::ImageLayout::GENERAL);

  // do the copy dispatch
  let pc = blob();
  // dst_offset
  pc.append_int(0);
  pc.append_int(0);

  // src_width/height
  pc.append_int(w);
  pc.append_int(h);

  // dst_width/height
  pc.append_int(w);
  pc.append_int(h);

  let x_size = 8;
  let y_size = 8;

  let workgroups = #{ x_groups: (w / x_size) + w % x_size,
                      y_groups: (h / y_size) + h % y_size,
                      z_groups: 1 };

  batch.dispatch_compute(self::img_pipeline.get(),
                         [self::out_desc_set.get(),
                          swapchain.storage_set],
                         pc,
                         workgroups);

  batch.transition_image(swapchain.img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::AccessFlags::SHADER_READ,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         // vk::AccessFlags::MEMORY_READ,
                         // vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                         vk::ImageLayout::GENERAL,
                         vk::ImageLayout::PRESENT_SRC_KHR);

  batch
}


fn draw_labels(batch, width, height, labels) {
  let w = width;
  let h = height;

  let x_size = 16;
  let y_size = 16;

  let x_groups = (w / x_size) + w % x_size;
  let y_groups = (h / y_size) + h % y_size;
  let workgroups = #{ x_groups: x_groups,
                      y_groups: y_groups,
                      z_groups: 1 };


  for label in labels {
    let pc = blob();
    pc.append_int(label.x);
    pc.append_int(label.y);
    pc.append_int(w);
    pc.append_int(h);

    batch.dispatch_compute(self::text_pipeline.get(),
                           [self::font_desc_set.get(),
                            label.desc_set,
                            self::out_desc_set.get()],
                           pc,
                           workgroups);


  }

  let img = self::out_image.get();
  batch.transition_image(img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::AccessFlags::SHADER_READ,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::ImageLayout::GENERAL,
                         vk::ImageLayout::GENERAL);

  batch
}

fn visible_slots(win_height) {
  ((win_height - 40.0) / 56.0).to_int()
}


fn slot_push_constant(win_dims, slot_ix) {
  let pc = blob();

  let yd = 66.0;

  let r = self::rect;
  r.y = r.y + yd * slot_ix;
  r.w = r.w + win_dims.width;

  pc.append_int(r.x.to_int());
  pc.append_int(r.y.to_int());
  pc.append_int(r.w.to_int());
  pc.append_int(r.h.to_int());
  pc
}

fn draw_slots(batch, win_dims, slot_dims, slot_desc_sets, color_buffer) {
  let x_size = 8;
  let y_size = 8;

  let slot_w = slot_dims.width;
  let slot_h = slot_dims.height;

  let workgroups = #{ x_groups: (slot_w / x_size) + slot_w % x_size,
                      y_groups: (slot_h / y_size) + slot_h % y_size,
                      z_groups: 1 };

  let color_data = get_buffer(color_buffer);

  let vis_count = visible_slots(win_dims.height);
  if vis_count >= slot_desc_sets.len() {
    vis_count = slot_desc_sets.len()
  }

  for ix in 0..vis_count {
    let desc_sets = slot_desc_sets[ix];

    let slot_set = desc_sets.slot;

    let pc = slot_push_constant(win_dims, ix);

    batch.dispatch_compute(self::path_pipeline.get(),
                           [slot_set,
                            // clip_set,
                            color_data.set,
                            self::out_desc_set.get()],
                           pc,
                           workgroups);
  }

  batch
}
