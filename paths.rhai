//
// configurable options
//

// colors and theming

import "color" as color;

export let bg_color = color::bg_light;
export let label_color = #{ r: 0.0, g: 0.0, b: 0.0 };

// alternative dark mode example
// export let bg_color = #{ r: 0.05, g: 0.05, b: 0.05 };
// export let label_color = #{ r: 0.85, g: 0.85, b: 0.85 };

export let cfg = config::new_config_map();

export let props = config::new_config_map();


slot::set_slot_color_scheme("loop_count_mean", "gradient-colorbrewer-spectral");
slot::set_slot_color_scheme("node_length", "gradient-grayscale");

// cfg.set("viz.slot_function", "node_length");
cfg.set("viz.slot_function", "loop_count_mean");
// cfg.set("viz.slot_function", "loop_count_mid");
// cfg.set("viz.slot_function", "has_node_mid");

// controls the slot offset and size, and the relative label offset
// TODO: the slot offset should be derived from the label length, but
// i'm not sure where to put the logic
// for now it's best to just change these values as needed
cfg.set("layout.slot", #{ x: 0, y: 36, w: -10, h: 16 });
cfg.set("layout.label", #{ x: 14, y: 2 });
cfg.set("layout.max_path_name_len", 14);
cfg.set("layout.padding", 2);


// this function can be edited to change the mapping from values to colors;
// the output is used an index into a 256-long gradient buffer
fn value_color_index_map(v) {
  if v <= 0.0 {
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
// keybinds and commands
//

let tgl_short_names = false;

const short_name_len = 11;
const long_name_len = 40;

bind_key(key::Tab, |pressed| {
  if pressed {
    cfg.update("layout.max_path_name_len", || {
      if tgl_short_names {
        this = short_name_len;
      } else {
        this = long_name_len;
      }
      tgl_short_names ^= true;
    });
  }
});


bind_key(key::F1, |pressed| {
  if pressed {
    cfg.update("viz.slot_function", || {
      if this == "node_length" {
        this = "loop_count_mean";
        slot::force_update();
      } else {
        this = "node_length";
        slot::force_update();
      }
    });
  }
});

//
// GUI definition
//

fn set_tooltip(text, pos) {
  // gui::set_layer_order(["main_gui", "main_gui-legend", "tooltip"]);
  // gui::set_layer_order(["main_gui", "tooltip"]);
  gui::set_layer_order(["tooltip"]);

  let width = text.len() * 8 + 4;

  // this makes the label centered above the given point (good for e.g. mouse pos)
  pos.x -= width / 2;
  pos.y -= 8;

  let inner = #{ x: pos.x - 2, y: pos.y - 2, w: width, h: 12, c: 3 };
  let outer = inner;

  outer.x -= 1;
  outer.w += 2;
  outer.y -= 1;
  outer.h += 2;
  outer.c = 2;

  gui::with_layer("tooltip", || {
    gui::set_rects(this, [outer, inner]);
  });

  let msg = gui::label("tooltip", "label");
  msg.contents = text;
  msg.position = pos;

  gui::update_label(msg);
}

fn remove_tooltip(text) {
  // gui::set_layer_order(["main_gui"]);
  gui::set_layer_order([]);
}



fn gui_init() {
  gui::set_layer_order(["main_gui"]);
}

props.set("prev_ix", false);

fn gui_update() {
  let props = viewer::props;

  let pos = mouse::get_pos();

  // set_tooltip(`(${pos.x}, ${pos.y})`, pos);

  let prev_ix = props.get("prev_ix");

  let ix = slot_ix_at_pos(pos);

  if prev_ix != ix {

    if ix.type_of() != "bool" {
      let list_range = slot::list_range();
      let path = graph::path(ix + list_range.start);

      if path.type_of() == "Path" {
        let path_name = path.name();
        print(`slot: ${ix} -- path: ${path_name}`);
      }
    }

    props.set("prev_ix", ix);

  }
}

/*
fn gui_init() {
  gui::set_layer_order(["main_gui", "main_gui-legend"]);
}

fn gui_update() {
  let pos = mouse::get_pos();
  let position = #{ x: pos.x, y: pos.y };

  set_tooltip(`(${pos.x.to_int()}, ${pos.y.to_int()})`, position);
  gui::with_layer("main_gui", Fn("prepare_legend_window"));
  gui::with_layer("main_gui-legend", Fn("prepare_legend_icons"));
}
*/

fn prepare_legend_icons() {
  let screen = gui::get_window_size();

  let values = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0];

  let y_delta = 40;

  let origin = #{ x: 40, y: 80 };

  let rect = origin;
  rect.w = 16;
  rect.h = 16;

  let label_origin = origin;
  label_origin.x += 20;
  label_origin.y += 4;

  let rects = values.map(|val, ix| {
    let label = gui::label("main_gui-legend", `legend-slot-${ix}`);
    let pos = label_origin;
    pos.y += ix * y_delta;
    label.position = pos;
    label.contents = `${val}`;
    gui::update_label(label);

    let color = value_color_index_map(val);

    let r = rect;

    r.y += ix * y_delta;

    r.c = color;

    r
  });

  gui::set_rects(this, rects);
}

fn prepare_legend_window() {
  let screen = gui::get_window_size();

  let top_left = #{ x: 20, y: 40 };

  // print(screen);
  let outer = top_left;
  outer.h = screen.height - 2 * top_left.y;
  outer.w = screen.width - 2 * top_left.x;

  outer.c = 6;
  // gui::set_rects(this, [window_outer]);

  let title_bar = outer;
  title_bar.y += 14;
  title_bar.h = 2;
  title_bar.x += 2;
  title_bar.w -= 4;
  title_bar.c = 4;

  let main = outer;

  main.y += 2;
  main.h -= 4;
  main.x += 2;
  main.w -= 4;
  main.c = 6;

  gui::set_rects(this, [outer, main, title_bar]);

  let title = gui::label("main_gui", "title");
  title.contents = "Help";
  title.position = #{ x: 24, y: 44 };
  gui::update_label(title);
}



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

  let cfg = viewer::cfg;

  let img = viewer::out_image.get();

  batch.transition_image(img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER | vk::PipelineStageFlags::FRAGMENT_SHADER,
                         vk::AccessFlags::SHADER_READ,
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
  let colors = slot::get_slot_color_scheme(cfg.get("viz.slot_function"));

  if colors.type_of() == "string" {
    batch = draw_slots(batch,
                           dims,
                           desc_sets,
                           colors);
  } else {
    print("could not find color scheme");
  }

  batch.transition_image(img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::AccessFlags::SHADER_READ,
                         vk::PipelineStageFlags::COMPUTE_SHADER | vk::PipelineStageFlags::FRAGMENT_SHADER,
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


fn draw_labels(width, height, labels) {
  let w = width;
  let h = height;

  let img = viewer::out_image.get();
  this.transition_image(img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER | vk::PipelineStageFlags::FRAGMENT_SHADER,
                         vk::AccessFlags::SHADER_READ,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::ImageLayout::GENERAL,
                         vk::ImageLayout::GENERAL);

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

    this.dispatch_compute(viewer::text_pipeline.get(),
                           [viewer::font_desc_set.get(),
                            label.desc_set,
                            viewer::out_desc_set.get()],
                           pc,
                           workgroups);


  }


  this.transition_image(img,
                         vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER,
                         vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE,
                         vk::PipelineStageFlags::COMPUTE_SHADER | vk::PipelineStageFlags::FRAGMENT_SHADER,
                         vk::ImageLayout::GENERAL,
                         vk::ImageLayout::GENERAL);

}

fn visible_slots(win_height) {
  let cfg = viewer::cfg;
  let slot = cfg.get("layout.slot");
  let padding = cfg.get("layout.padding");

  ((win_height - slot.y) / (slot.h + padding)).to_int()
}


fn slot_rect(win_dims, slot_ix) {
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

  let yd = slot.h + padding;
  let r = slot;

  r.x = slot_x;
  r.y = r.y + yd * slot_ix;
  r.w = slot_w;

  r
}

fn slot_rect(win_dims) {
  slot_rect(win_dims, 0)
}

fn slot_ix_at_pos(pos) {
  let cfg = viewer::cfg;
  let padding = cfg.get("layout.padding");

  let win_dims = gui::get_window_size();
  let rect = slot_rect(win_dims);

  // let vis_count = ((win_dims.height - rect.y) / (rect.h + padding)).to_int();

  let yd = rect.h + padding;
  let y0 = rect.y;

  let x = pos.x.to_int();
  let y = pos.y.to_int();

  if pos.y < y0 || pos.x < rect.x || pos.x > rect.x + rect.w {
    return false;
  }

  let y = pos.y - y0;

  if pos.y % yd > rect.h {
    return false;
  }

  let i = y / yd;

  // if i > vis_count {
  //   return false;
  // }

  i.to_int()
}

fn draw_slots(batch, win_dims, slot_desc_sets, color_buffer) {
  let x_size = 8;
  let y_size = 8;

  let cfg = viewer::cfg;
  let slot = cfg.get("layout.slot");

  let slot = slot_rect(win_dims, 0);

  let workgroups = #{ x_groups: (slot.w / x_size) + slot.w % x_size,
                      y_groups: (slot.h / y_size) + slot.h % y_size,
                      z_groups: 1 };

  if slot.w > 0 && slot.h > 0 {
    try {
      let color_data = get_buffer(color_buffer);

      let vis_count = visible_slots(win_dims.height);
      if vis_count >= slot_desc_sets.len() {
        vis_count = slot_desc_sets.len()
      }

      for ix in 0..vis_count {
        let desc_sets = slot_desc_sets[ix];

        let slot_set = desc_sets.slot;

        if slot_set.type_of() != "()" {
          let rect = slot_rect(win_dims, ix);
          let pc = blob();
          pc.append_int(rect.x.to_int());
          pc.append_int(rect.y.to_int());
          pc.append_int(rect.w.to_int());
          pc.append_int(rect.h.to_int());

          batch.dispatch_compute(viewer::path_pipeline.get(),
                                 [slot_set,
                                  color_data.set,
                                  viewer::out_desc_set.get()],
                                 pc,
                                 workgroups);
        } else {
          print("got a () in draw_slots!");
        }

      }
    }
    catch (err)
    {
      print(err);
    }
  }

  batch
}
