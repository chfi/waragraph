//
// configurable options
//

// colors and theming

import "color" as color;
import "script/tooltip" as tooltip;
import "script/ruler" as ruler;

export let bg_color = color::bg_light;
export let label_color = #{ r: 0.0, g: 0.0, b: 0.0 };

// alternative dark mode example
// export let bg_color = #{ r: 0.05, g: 0.05, b: 0.05 };
// export let label_color = #{ r: 0.85, g: 0.85, b: 0.85 };

export let cfg = config::new_config_map();

export let props = config::new_config_map();


slot::set_slot_color_scheme("depth_mean", "gradient-colorbrewer-spectral");
slot::set_slot_color_scheme("node_length", "gradient-grayscale");

// cfg.set("viz.slot_function", "node_length");
cfg.set("viz.slot_function", "depth_mean");
// cfg.set("viz.slot_function", "depth_mid");
// cfg.set("viz.slot_function", "has_node_mid");

// controls the slot offset and size, and the relative label offset
// TODO: the slot offset should be derived from the label length, but
// i'm not sure where to put the logic
// for now it's best to just change these values as needed
cfg.set("layout.slot", #{ x: 0, y: 36, w: -10, h: 16 });
cfg.set("layout.list_bottom_pad", 100);
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

props.set("viz.previous", "path_position");

bind_key(key::F2, |pressed| {
  let cfg = viewer::cfg;
  let props = viewer::props;
  let slot_fns = slot::slot_fn_names();

  if pressed {
    popup::popup(slot_fns, |fn_name| {
      cfg.update("viz.slot_function", || {
        props.set("viz.previous", this);
        this = fn_name;
      })
    });
  }
});

bind_key(key::F1, |pressed| {
  let cfg = viewer::cfg;
  let props = viewer::props;

  if pressed {
    let cur = cfg.get("viz.slot_function");
    let prev = props.get("viz.previous");

    props.set("viz.previous", cur);
    cfg.set("viz.slot_function", prev);
    slot::force_update();
  }
});

/*
// disabled since it's broken for now
bind_key(key::F9, |pressed| {
  if pressed {
    let scale = slot::get_scale_factor();
    if scale == 1 {
      slot::set_scale_factor(2);
    } else if scale == 2 {
      slot::set_scale_factor(4);
    } else {
      slot::set_scale_factor(1);
    }
  }
});
*/

//
// GUI definition
//


fn remove_tooltip() {
  ui::toggle_layer("tooltip", false);
}


fn gui_init(globals, label_space) {

  globals.tooltip_data = #{};

  let props = viewer::props;
  props.set("time", 0.0);

  ui::toggle_layer("tree-list-layer", false);

  tooltip::init_tooltip(label_space);

  ruler::init_ruler(label_space);

  ui::init_layer("lines", 5, true);
  ui::allocate_sublayer("lines", "line-rgb", "lines");

  ui::init_layer("edges", 10, true);
  ui::allocate_sublayer("edges", "line-rgb", "lines");
  ui::allocate_sublayer("edges", "rect-rgb", "rects");
}

props.set("prev_slot", false);

fn rgb(r, g, b) {
#{ r: r, g: g, b: b, a: 1.0 }
}

fn rgba(r, g, b, a) {
#{ r: r, g: g, b: b, a: 1.0 }
}

fn append_rect(x, y, w, h, color) {
  this.push(#{ x: x, y: y,
               w: w, h: h,
               r: color.r, g: color.g, b: color.b, a: color.a });
}

fn line(x0, y0, x1, y1) {
#{ x0: x0, y0: y0, w0: 1.0,
   x1: x1, y1: y1, w1: 1.0,
 } + rgb(1.0, 0.0, 0.0)
}

props.set("hovered_path", ());

// fn gui_update() {
//   import "color" as color;
//   import "script/tooltip" as tooltip;
//   import "script/ruler" as ruler;

fn gui_update(globals, label_space, dt) {

  let props = viewer::props;
  let g_time = props.get("time");

  props.update("time", || this += dt);

  let pos = mouse::get_pos();

  let pos = mouse::get_pos();
  let view = viewer::get_view();
  let dims = ui::get_window_size();

  let ruler_rect = slot_rect(dims);
  ruler_rect.y -= 8;

  ruler::draw_ruler(label_space, ruler_rect, view);

  /*
  {
    let l0 = line(100.0, 100.0, pos.x, pos.y);
    let l1 = line(dims.width - 100.0, 100.0, pos.x, pos.y);
    let l2 = line(dims.width - 100.0, dims.height - 100.0, pos.x, pos.y);
    let l3 = line(100.0, dims.height - 100.0, pos.x, pos.y);

    l0.w0 = 4.0;
    l1.w0 = 4.0;
    l2.w0 = 4.0;
    l3.w0 = 4.0;

    ui::update_sublayer("lines", "lines", [l0, l1, l2, l3]);
  }
  */

  let prev_slot = props.get("prev_slot");

  let slot = slot_ix_at_pos(pos);

  if slot == false {
    remove_tooltip();
  }

  if prev_slot != slot {

    if slot.type_of() != "bool" {
      let list_range = slot::list_range();
      let path = graph::path(slot.index + list_range.start);

      let offset = view.offset.to_float();
      let len = view.len.to_float();

      let bp_pos = (slot.t * len + offset).to_int();

      if path.type_of() == "Path" {
        let path_name = path.name();
        let node = graph::node_at_pos(bp_pos);
        let path_pos = graph::pos_at_node(path, node);

        props.set("hovered_path", path);

        // print(path_name);

        // print(globals.hovered_path);

        let text = `${node} - ${path_pos}`;

        let slot_pos = slot_rect(dims, slot.index);

        props.set("hovered_slot_y", slot_pos.y + slot_pos.h);

        let rows = [`Node ${node}`];

        // kind of hacky, but this is where the BED-related
        // `tooltip_data` is consumed (see script/tooltip.rhai)
        try {
          let data_sources = globals.tooltip_data;
          if data_sources.type_of() != "array" {
            throw "TooltipNotArray";
          }

          for tooltip in data_sources {
            let row_ix = tooltip.ix_data_source.at(path, node);
            if row_ix.type_of() == "i64" {

              let val = tooltip.bed.get_record_field(row_ix, tooltip.col);

              let text = `${tooltip.name} ${val}`;
              rows.push(text);

              print(val);
            }
          }
        }
        catch (e) {
          if e != "TooltipNotArray" {
               print(e);
          }
        }

        if path_name != false {
          rows.push(`Path ${path_name}`);

          if path_pos != false {
            rows.push(` Pos ${path_pos}`);
          }
        }

        tooltip::show_tooltip(label_space, pos, rows);

        print(`pos: ${pos} -- node: ${node} -- path: ${path_name}, pos: ${path_pos}`);
        clipboard::set_text(`pos: ${pos} -- node: ${node} -- path: ${path_name}, pos: ${path_pos}`);
      }
    }

    props.set("prev_slot", slot);

  }
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

// text

export let text_shader = load_shader("shaders/text.comp.spv",
                                     vk::ShaderStageFlags::VERTEX
                                     | vk::ShaderStageFlags::COMPUTE
                                     | vk::ShaderStageFlags::FRAGMENT);
export let text_pipeline = create_compute_pipeline(text_shader);


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
                                     vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::COMPUTE | vk::ShaderStageFlags::FRAGMENT);
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

export let img_shader =
  load_shader("shaders/copy_image.comp.spv",
              vk::ShaderStageFlags::VERTEX
              | vk::ShaderStageFlags::FRAGMENT
              | vk::ShaderStageFlags::COMPUTE);

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
                         vk::ImageLayout::GENERAL,
                         vk::ImageLayout::PRESENT_SRC_KHR);

  batch
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

  let win_dims = ui::get_window_size();
  let rect = slot_rect(win_dims);

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

  let t = (pos.x - rect.x) / rect.w;

  // if i > vis_count {
  //   return false;
  // }

  return #{ index: i.to_int(), t: t };
}
