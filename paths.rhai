//
// configurable options
//

// colors and theming

import "color" as color;

// export let bg_color = #{ r: 0.95, g: 0.95, b: 0.95 };
export let bg_color = color::bg_light;
// export let slot_gradient = "gradient_spectral";
export let slot_gradient = "gradient-test";
export let label_color = #{ r: 0.0, g: 0.0, b: 0.0 };

// alternative dark mode example
// export let bg_color = #{ r: 0.05, g: 0.05, b: 0.05 };
// export let slot_gradient = "gradient_inferno";
// export let label_color = #{ r: 0.85, g: 0.85, b: 0.85 };

export let cfg = config::new_config_map();

// controls the slot offset and size, and the relative label offset
// TODO: the slot offset should be derived from the label length, but
// i'm not sure where to put the logic
// for now it's best to just change these values as needed
cfg.set("layout.slot", #{ x: 0, y: 36, w: -10, h: 16 });
cfg.set("layout.label", #{ x: 14, y: 40 });
cfg.set("layout.max_path_name_len", 14);
cfg.set("layout.padding", 4);

// other options
export let slot_function = "loop_count_mean";
// export let slot_function = "loop_count_mid";
// export let slot_function = "has_node_mid";

// this function can be edited to change the mapping from values to colors;
// the output is used an index into a 256-long gradient buffer
fn value_color_index_map(v) {
  if v <= 0.01 {
    0
  } else {
    v += 0.5;

    if v > 255.0 {
      255
    } else {
      v.to_int() + 1
    }
  }
}

// fn value_color_index_map(v) {
//   if v <= 0.5 {
//     0
//   } else if v < 1.5 {
//     16
//   } else if v < 3.0 {
//     32
//   } else if v < 4.5 {
//     64
//   } else {
//     128
//   }
// }

//
// engine-related resources
//

// these are bound to the WindowResources' "out" image in
// ViewerSys::init, in src/viewer/app.rs
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

let counter = 0;

bind_key(key::A, |pressed| {
  if pressed {
    print("pressed");
    counter += 1;
  } else {
    print(`counter: ${counter}`);
  }
});


fn init() {
  let init_batch = batch_builder();

  init_batch.load_image_from_file("8x8font.png",
                                  viewer::font_image.get(),
                                  vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

  let img = viewer::out_image.get();

  init_batch.transition_image(viewer::out_image.get(),
                              vk::AccessFlags::empty(),
                              vk::PipelineStageFlags::TOP_OF_PIPE,
                              vk::AccessFlags::SHADER_READ
                              | vk::AccessFlags::SHADER_WRITE,
                              vk::PipelineStageFlags::COMPUTE_SHADER,
                              vk::ImageLayout::UNDEFINED,
                              vk::ImageLayout::GENERAL);

  init_batch
}


// called when the window is resized
fn resize(width, height) {
  let init_batch = batch_builder();

  let img = viewer::out_image.get();

  init_batch.transition_image(viewer::out_image.get(),
                              vk::AccessFlags::empty(),
                              vk::PipelineStageFlags::TOP_OF_PIPE,
                              vk::AccessFlags::SHADER_READ
                              | vk::AccessFlags::SHADER_WRITE,
                              vk::PipelineStageFlags::COMPUTE_SHADER,
                              vk::ImageLayout::UNDEFINED,
                              vk::ImageLayout::GENERAL);

  init_batch
}


fn foreground(batch, desc_sets, w, h) {

  let img = viewer::out_image.get();

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
  pc.append_float(viewer::bg_color.r);
  pc.append_float(viewer::bg_color.g);
  pc.append_float(viewer::bg_color.b);
  pc.append_float(1.0);
  pc.append_int(w);
  pc.append_int(h);

  let x_size = 16;
  let y_size = 16;

  let x_groups = (w / x_size) + w % x_size;
  let y_groups = (h / y_size) + h % y_size;

  batch.dispatch_compute(viewer::bg_pipeline.get(),
                         viewer::out_desc_set.get(),
                         // viewer::bg_desc_set.get(),
                         pc, x_groups, y_groups, 1);



  // draw paths
  let dims = #{ width: w, height: h };
  let batch = draw_slots(batch,
                         dims,
                         desc_sets,
                         viewer::slot_gradient);
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


  batch.dispatch_compute(viewer::line_pipeline.get(),
                         [line_data.set,
                          viewer::out_desc_set.get()],
                         pc,
                         workgroups);
                         */


  // add a memory barrier (may not be needed, but)
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

  batch.dispatch_compute(viewer::img_pipeline.get(),
                         [viewer::out_desc_set.get(),
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

  for label in labels {
    let pc = blob();
    pc.append_int(label.x);
    pc.append_int(label.y);
    pc.append_int(w);
    pc.append_int(h);

    pc.append_float(viewer::label_color.r);
    pc.append_float(viewer::label_color.g);
    pc.append_float(viewer::label_color.b);
    pc.append_float(1.0);

    let workgroups = #{ x_groups: label.len,
                        y_groups: 1,
                        z_groups: 1 };

    batch.dispatch_compute(viewer::text_pipeline.get(),
                           [viewer::font_desc_set.get(),
                            label.desc_set,
                            viewer::out_desc_set.get()],
                           pc,
                           workgroups);


  }

  let img = viewer::out_image.get();
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
  let cfg = viewer::cfg;
  let slot = cfg.get("layout.slot");
  let padding = cfg.get("layout.padding");

  ((win_height - slot.y) / (slot.h + padding)).to_int()
}

fn slot_push_constant(win_dims, slot_x, slot_w, slot_ix) {
  let pc = blob();

  let cfg = viewer::cfg;
  let slot = cfg.get("layout.slot");
  let padding = cfg.get("layout.padding");

  let path_name_len = cfg.get("layout.max_path_name_len");

  let yd = slot.h + padding;
  let r = slot;

  r.x = slot_x;
  r.y = r.y + yd * slot_ix;
  r.w = slot_w;

  pc.append_int(r.x.to_int());
  pc.append_int(r.y.to_int());
  pc.append_int(r.w.to_int());
  pc.append_int(r.h.to_int());
  pc
}

fn draw_slots(batch, win_dims, slot_desc_sets, color_buffer) {
  let x_size = 8;
  let y_size = 8;

  let cfg = viewer::cfg;
  let slot = cfg.get("layout.slot");

  let label = cfg.get("layout.label");
  let path_name_len = cfg.get("layout.max_path_name_len");
  let padding = cfg.get("layout.padding");

  let slot_x = slot.x + label.x + padding + path_name_len * 8;

  let slot_w = if slot.w < 0 {
    win_dims.width + slot.w - slot_x
  } else {
    slot.w
  };
  let slot_h = slot.h;

  let workgroups = #{ x_groups: (slot_w / x_size) + slot_w % x_size,
                      y_groups: (slot_h / y_size) + slot_h % y_size,
                      z_groups: 1 };

  try {
    let color_data = get_buffer(color_buffer);

    let vis_count = visible_slots(win_dims.height);
    if vis_count >= slot_desc_sets.len() {
      vis_count = slot_desc_sets.len()
    }

    for ix in 0..vis_count {
      let desc_sets = slot_desc_sets[ix];

      let slot_set = desc_sets.slot;

      let pc = slot_push_constant(win_dims, slot_x, slot_w, ix);

      batch.dispatch_compute(viewer::path_pipeline.get(),
                             [slot_set,
                              color_data.set,
                              viewer::out_desc_set.get()],
                             pc,
                             workgroups);
    }
  }
  catch (err)
  {
    print(err);
  }

  batch
}
