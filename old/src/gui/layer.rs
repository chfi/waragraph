use parking_lot::RwLock;
use raving::compositor::label_space::LabelSpace;
use raving::vk::context::VkContext;
use raving::vk::{BufferIx, DescSetIx, GpuResources, ImageIx, VkEngine, ImageViewIx};

use raving::compositor::*;

use ash::vk;

use crate::geometry::{ScreenPoint, ScreenRect};

use std::sync::Arc;

use anyhow::Result;

use zerocopy::AsBytes;

pub mod image;

pub use image::*;

// vertex constructor helpers

pub fn rect_rgba(rect: ScreenRect, rgba: rgb::RGBA<f32>) -> [u8; 32] {
    let mut out = [0u8; 32];

    let x = rect.min_x();
    let y = rect.min_y();

    let w = rect.width();
    let h = rect.height();

    out[0..8].clone_from_slice([x, y].as_bytes());
    out[8..16].clone_from_slice([w, h].as_bytes());
    out[16..32].clone_from_slice([rgba.r, rgba.g, rgba.b, rgba.a].as_bytes());

    out
}

pub fn line_rgba(
    p0: ScreenPoint,
    p1: ScreenPoint,
    rgba: rgb::RGBA<f32>,
) -> [u8; 40] {
    line_width_rgba(p0, p1, 0.5, 0.5, rgba)
}

pub fn line_width_rgba(
    p0: ScreenPoint,
    p1: ScreenPoint,
    w0: f32,
    w1: f32,
    rgba: rgb::RGBA<f32>,
) -> [u8; 40] {
    let mut out = [0u8; 40];

    out[0..12].clone_from_slice([p0.x, p0.y, w0].as_bytes());
    out[12..24].clone_from_slice([p1.x, p1.y, w1].as_bytes());
    out[24..40].clone_from_slice([rgba.r, rgba.g, rgba.b, rgba.a].as_bytes());

    out
}

pub fn line_width_rgba2(
    p0: ScreenPoint,
    p1: ScreenPoint,
    w0: f32,
    w1: f32,
    rgba0: rgb::RGBA<f32>,
    rgba1: rgb::RGBA<f32>,
) -> [u8; 56] {
    let mut out = [0u8; 56];

    out[0..12].clone_from_slice([p0.x, p0.y, w0].as_bytes());
    out[12..24].clone_from_slice([p1.x, p1.y, w1].as_bytes());
    out[24..40]
        .clone_from_slice([rgba0.r, rgba0.g, rgba0.b, rgba0.a].as_bytes());
    out[40..56]
        .clone_from_slice([rgba1.r, rgba1.g, rgba1.b, rgba1.a].as_bytes());

    out
}

pub fn label_at(
    p: ScreenPoint,
    text_range: (usize, usize),
    rgba: rgb::RGBA<f32>,
) -> [u8; 32] {
    let mut out = [0u8; 32];

    let x = p.x;
    let y = p.y;

    let (s, l) = text_range;

    out[0..8].clone_from_slice([x, y].as_bytes());
    out[8..16].clone_from_slice([s as u32, l as u32].as_bytes());
    out[16..32].clone_from_slice([rgba.r, rgba.g, rgba.b, rgba.a].as_bytes());

    out
}

pub fn path_slot(rect: ScreenRect, offset: usize, len: usize) -> [u8; 24] {
    let mut out = [0u8; 24];

    let p = rect.origin;
    let s = rect.size;

    let offset = offset as u32;
    let len = len as u32;

    out[0..8].clone_from_slice([p.x, p.y].as_bytes());
    out[8..16].clone_from_slice([s.width, s.height].as_bytes());
    out[16..24].clone_from_slice([offset, len].as_bytes());

    out
}

//

pub fn add_sublayer_defs(
    engine: &mut VkEngine,
    compositor: &mut Compositor,
    font_desc_set: DescSetIx,
) -> Result<()> {
    engine.with_allocators(|ctx, res, _| {
        let clear_pass = res[compositor.clear_pass];
        let load_pass = res[compositor.load_pass];
        compositor.add_sublayer_defs([
            text_sublayer(ctx, res, font_desc_set, clear_pass, load_pass)?,
            rect_rgb_sublayer(ctx, res, clear_pass, load_pass)?,
            line_rgb_sublayer(ctx, res, clear_pass, load_pass)?,
            line_rgb_2_sublayer(ctx, res, clear_pass, load_pass)?,
            slot::sublayer(ctx, res, clear_pass, load_pass)?,
            crate::text::glyph_sublayer(ctx, res, clear_pass, load_pass)?,
            crate::geometry::graph::sublayer::sublayer_def(
                ctx, res, clear_pass, load_pass,
            )?,
            image_sublayer(ctx, res, clear_pass, load_pass)?,
        ]);

        Ok(())
    })
}

pub(super) fn text_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    font_desc_set: DescSetIx,
    clear_pass: vk::RenderPass,
    load_pass: vk::RenderPass,
) -> Result<SublayerDef> {
    let vert =
        res.load_shader("shaders/text.vert.spv", vk::ShaderStageFlags::VERTEX)?;
    let frag = res.load_shader(
        "shaders/text.frag.spv",
        vk::ShaderStageFlags::VERTEX
            | vk::ShaderStageFlags::COMPUTE
            | vk::ShaderStageFlags::FRAGMENT,
    )?;

    let vert = res.insert_shader(vert);
    let frag = res.insert_shader(frag);

    let vert_binding_desc = vk::VertexInputBindingDescription::builder()
        .binding(0)
        .stride(std::mem::size_of::<[f32; 8]>() as u32)
        .input_rate(vk::VertexInputRate::INSTANCE)
        .build();

    let pos_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(0)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(0)
        .build();

    let ix_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(1)
        .format(vk::Format::R32G32_UINT)
        .offset(8)
        .build();

    let color_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(2)
        .format(vk::Format::R32G32B32A32_SFLOAT)
        .offset(16)
        .build();

    let vert_binding_descs = [vert_binding_desc];
    let vert_attr_descs = [pos_desc, ix_desc, color_desc];

    let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&vert_binding_descs)
        .vertex_attribute_descriptions(&vert_attr_descs);

    let vertex_offset = 0;
    let vertex_stride = 32;

    SublayerDef::new::<([f32; 2], [u32; 2], [f32; 4]), _>(
        ctx,
        res,
        "text",
        vert,
        frag,
        clear_pass,
        load_pass,
        vertex_offset,
        vertex_stride,
        true,
        Some(6),
        None,
        vert_input_info,
        None,
        [font_desc_set],
    )
}

pub(super) fn rect_rgb_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    clear_pass: vk::RenderPass,
    load_pass: vk::RenderPass,
) -> Result<SublayerDef> {
    let vert = res.load_shader(
        "shaders/rect_window.vert.spv",
        vk::ShaderStageFlags::VERTEX,
    )?;
    let frag = res.load_shader(
        "shaders/rect_window.frag.spv",
        vk::ShaderStageFlags::FRAGMENT,
    )?;

    let vert = res.insert_shader(vert);
    let frag = res.insert_shader(frag);

    let vert_binding_desc = vk::VertexInputBindingDescription::builder()
        .binding(0)
        .stride(std::mem::size_of::<[f32; 8]>() as u32)
        .input_rate(vk::VertexInputRate::INSTANCE)
        .build();

    let pos_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(0)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(0)
        .build();

    let size_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(1)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(8)
        .build();

    let color_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(2)
        .format(vk::Format::R32G32B32A32_SFLOAT)
        .offset(16)
        .build();

    let vert_binding_descs = [vert_binding_desc];
    let vert_attr_descs = [pos_desc, size_desc, color_desc];

    let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&vert_binding_descs)
        .vertex_attribute_descriptions(&vert_attr_descs);

    let vertex_offset = 0;
    let vertex_stride = 32;

    let mut def = SublayerDef::new::<([f32; 2], [f32; 2], [f32; 4]), _>(
        ctx,
        res,
        "rect-rgb",
        vert,
        frag,
        clear_pass,
        load_pass,
        vertex_offset,
        vertex_stride,
        true,
        Some(6),
        None,
        vert_input_info,
        None,
        None,
    )?;

    fn get_cast(map: &rhai::Map, k: &str) -> Option<f32> {
        let field = map.get(k)?;
        field
            .as_float()
            .ok()
            .or(field.as_int().ok().map(|v| v as f32))
    }

    def.set_parser(|map, out| {
        let x = get_cast(&map, "x")?;
        let y = get_cast(&map, "y")?;
        let w = get_cast(&map, "w")?;
        let h = get_cast(&map, "h")?;

        let r = get_cast(&map, "r")?;
        let g = get_cast(&map, "g")?;
        let b = get_cast(&map, "b")?;
        let a = get_cast(&map, "a")?;

        out[0..8].clone_from_slice([x, y].as_bytes());
        out[8..16].clone_from_slice([w, h].as_bytes());
        out[16..32].clone_from_slice([r, g, b, a].as_bytes());
        Some(())
    });

    Ok(def)
}

pub(super) fn line_rgb_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    clear_pass: vk::RenderPass,
    load_pass: vk::RenderPass,
) -> Result<SublayerDef> {
    let vert = res
        .load_shader("shaders/vector.vert.spv", vk::ShaderStageFlags::VERTEX)?;
    let frag = res.load_shader(
        "shaders/vector.frag.spv",
        vk::ShaderStageFlags::FRAGMENT,
    )?;

    let vert = res.insert_shader(vert);
    let frag = res.insert_shader(frag);

    let vertex_size = std::mem::size_of::<[f32; 10]>() as u32;

    let vert_binding_desc = vk::VertexInputBindingDescription::builder()
        .binding(0)
        .stride(vertex_size)
        .input_rate(vk::VertexInputRate::INSTANCE)
        .build();

    let p0_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(0)
        .format(vk::Format::R32G32B32_SFLOAT)
        .offset(0)
        .build();

    let p1_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(1)
        .format(vk::Format::R32G32B32_SFLOAT)
        .offset(12)
        .build();

    let color_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(2)
        .format(vk::Format::R32G32B32A32_SFLOAT)
        .offset(24)
        .build();

    let vert_binding_descs = [vert_binding_desc];
    let vert_attr_descs = [p0_desc, p1_desc, color_desc];

    let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&vert_binding_descs)
        .vertex_attribute_descriptions(&vert_attr_descs);

    let vertex_offset = 0;
    let vertex_stride = vertex_size as usize;

    let mut def = SublayerDef::new::<([f32; 3], [f32; 3], [f32; 4]), _>(
        ctx,
        res,
        "line-rgb",
        vert,
        frag,
        clear_pass,
        load_pass,
        vertex_offset,
        vertex_stride,
        true,
        Some(6),
        None,
        vert_input_info,
        None,
        None,
    )?;

    fn get_cast(map: &rhai::Map, k: &str) -> Option<f32> {
        let field = map.get(k)?;
        field
            .as_float()
            .ok()
            .or(field.as_int().ok().map(|v| v as f32))
    }

    def.set_parser(|map, out| {
        // let map = val.try_cast::<rhai::Map>()?;
        // let mut out = [0u8; 40];

        let x0 = get_cast(&map, "x0")?;
        let y0 = get_cast(&map, "y0")?;
        let x1 = get_cast(&map, "x1")?;
        let y1 = get_cast(&map, "y1")?;

        let w0 = get_cast(&map, "w0")?;
        let w1 = get_cast(&map, "w1")?;

        let r = get_cast(&map, "r")?;
        let g = get_cast(&map, "g")?;
        let b = get_cast(&map, "b")?;
        let a = get_cast(&map, "a")?;

        out[0..12].clone_from_slice([x0, y0, w0].as_bytes());
        out[12..24].clone_from_slice([x1, y1, w1].as_bytes());
        out[24..40].clone_from_slice([r, g, b, a].as_bytes());

        Some(())
    });

    Ok(def)
}

pub(super) fn line_rgb_2_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    clear_pass: vk::RenderPass,
    load_pass: vk::RenderPass,
) -> Result<SublayerDef> {
    let vert = res.load_shader(
        "shaders/vector_2color.vert.spv",
        vk::ShaderStageFlags::VERTEX,
    )?;
    let frag = res.load_shader(
        "shaders/vector.frag.spv",
        vk::ShaderStageFlags::FRAGMENT,
    )?;

    let vert = res.insert_shader(vert);
    let frag = res.insert_shader(frag);

    let vertex_size = std::mem::size_of::<[f32; 14]>() as u32;

    let vert_binding_desc = vk::VertexInputBindingDescription::builder()
        .binding(0)
        .stride(vertex_size)
        .input_rate(vk::VertexInputRate::INSTANCE)
        .build();

    let p0_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(0)
        .format(vk::Format::R32G32B32_SFLOAT)
        .offset(0)
        .build();

    let p1_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(1)
        .format(vk::Format::R32G32B32_SFLOAT)
        .offset(12)
        .build();

    let color0_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(2)
        .format(vk::Format::R32G32B32A32_SFLOAT)
        .offset(24)
        .build();

    let color1_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(3)
        .format(vk::Format::R32G32B32A32_SFLOAT)
        .offset(40)
        .build();

    let vert_binding_descs = [vert_binding_desc];
    let vert_attr_descs = [p0_desc, p1_desc, color0_desc, color1_desc];

    let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&vert_binding_descs)
        .vertex_attribute_descriptions(&vert_attr_descs);

    let vertex_offset = 0;
    let vertex_stride = vertex_size as usize;

    let mut def =
        SublayerDef::new::<([f32; 3], [f32; 3], [f32; 4], [f32; 4]), _>(
            ctx,
            res,
            "line-rgb-2",
            vert,
            frag,
            clear_pass,
            load_pass,
            vertex_offset,
            vertex_stride,
            true,
            Some(6),
            None,
            vert_input_info,
            None,
            None,
        )?;

    fn get_cast(map: &rhai::Map, k: &str) -> Option<f32> {
        let field = map.get(k)?;
        field
            .as_float()
            .ok()
            .or(field.as_int().ok().map(|v| v as f32))
    }

    def.set_parser(|map, out| {
        // let map = val.try_cast::<rhai::Map>()?;
        // let mut out = [0u8; 40];

        let x0 = get_cast(&map, "x0")?;
        let y0 = get_cast(&map, "y0")?;
        let x1 = get_cast(&map, "x1")?;
        let y1 = get_cast(&map, "y1")?;

        let w0 = get_cast(&map, "w0")?;
        let w1 = get_cast(&map, "w1")?;

        let r0 = get_cast(&map, "r0")?;
        let g0 = get_cast(&map, "g0")?;
        let b0 = get_cast(&map, "b0")?;
        let a0 = get_cast(&map, "a0")?;

        let r1 = get_cast(&map, "r1")?;
        let g1 = get_cast(&map, "g1")?;
        let b1 = get_cast(&map, "b1")?;
        let a1 = get_cast(&map, "a1")?;

        out[0..12].clone_from_slice([x0, y0, w0].as_bytes());
        out[12..24].clone_from_slice([x1, y1, w1].as_bytes());
        out[24..40].clone_from_slice([r0, g0, b0, a0].as_bytes());
        out[40..56].clone_from_slice([r1, g1, b1, a1].as_bytes());

        Some(())
    });

    Ok(def)
}

pub fn create_rhai_module(compositor: &Compositor) -> rhai::Module {
    let mut module = raving::compositor::create_rhai_module(compositor);

    // TODO: this implementation might lead to issues if the
    // swapchain is resized before/while the script is running,
    // if/when scripts are being run asynchronously
    let window_size = compositor.window_dims_arc().clone();
    module.set_native_fn("get_window_size", move || {
        let [x, y] = window_size.load();
        let mut map = rhai::Map::default();
        map.insert("width".into(), (x as i64).into());
        map.insert("height".into(), (y as i64).into());
        Ok(map)
    });

    let layers = compositor.layers.clone();

    module.set_native_fn(
        "init_layer",
        move |name: &str, depth: i64, enabled: bool| {
            let mut layers = layers.write();
            if layers.contains_key(name) {
                return Ok(rhai::Dynamic::FALSE);
            }

            layers.insert(name.into(), Layer::new(depth as usize, enabled));

            Ok(rhai::Dynamic::TRUE)
        },
    );

    let alloc_tx = compositor.sublayer_alloc_tx.clone();

    module.set_native_fn(
        "allocate_rect_sublayer",
        move |layer_name: &str, sublayer_name: &str| {
            let msg = SublayerAllocMsg::new(
                layer_name,
                sublayer_name,
                "rect-rgb",
                &[],
            );

            if let Err(e) = alloc_tx.send(msg) {
                Err(format!("sublayer allocation message error: {:?}", e)
                    .into())
            } else {
                Ok(())
            }
        },
    );

    let alloc_tx = compositor.sublayer_alloc_tx.clone();

    module.set_native_fn(
        "allocate_text_sublayer",
        move |label_space: &mut Arc<RwLock<LabelSpace>>,
              layer_name: &str,
              sublayer_name: &str| {
            let text_set = label_space.read().text_set;

            let msg = SublayerAllocMsg::new(
                layer_name,
                sublayer_name,
                "text",
                &[text_set],
            );

            if let Err(e) = alloc_tx.send(msg) {
                Err(format!("sublayer allocation message error: {:?}", e)
                    .into())
            } else {
                Ok(())
            }
        },
    );

    let layers = compositor.layers.clone();
    module.set_native_fn(
        "toggle_layer",
        move |layer_name: &str, enabled: bool| {
            let mut layers = layers.write();
            if let Some(layer) = layers.get_mut(layer_name) {
                layer.enabled = enabled;
                return Ok(layer.enabled);
            }

            Ok(false)
        },
    );

    let layers = compositor.layers.clone();

    module.set_native_fn(
        "update_sublayer",
        move |label_space: &mut Arc<RwLock<LabelSpace>>,
              layer_name: &str,
              sublayer_name: &str,
              labels: rhai::Array| {
            let mut layers = layers.write();

            if let Some(layer) = layers.get_mut(layer_name) {
                if let Some(sublayer) = layer.get_sublayer_mut(sublayer_name) {
                    match sublayer.def_name.as_str() {
                        "text" => {
                            let vertices =
                                super::tree_list::rhai_module::label_rects(
                                    label_space,
                                    labels,
                                )?;

                            let result =
                                sublayer.draw_data_mut().try_for_each(|data| {
                                    data.update_vertices_array(
                                        vertices.iter().cloned(),
                                    )
                                });

                            if let Err(e) = result {
                                return Err(format!(
                                    "sublayer update error: {:?}",
                                    e
                                )
                                .into());
                            } else {
                                return Ok(());
                            }
                        }
                        e => {
                            return Err(format!(
                                "expected `text` sublayer type: `{}`",
                                e
                            )
                            .into());
                        }
                    }
                }
            }

            Ok(())
        },
    );

    let layers = compositor.layers.clone();

    module.set_native_fn(
        "update_sublayer",
        move |layer_name: &str, sublayer_name: &str, data: rhai::Array| {
            let mut layers = layers.write();

            if let Some(layer) = layers.get_mut(layer_name) {
                if let Some(sublayer) = layer.get_sublayer_mut(sublayer_name) {
                    let parser =
                        sublayer.def.parse_rhai_vertex.clone().unwrap();

                    let def_name = sublayer.def_name.clone();
                    // TODO this only updates the first draw data set for now
                    let draw_data = sublayer.draw_data_mut().next().unwrap();

                    // need a macro since arrays have fixed length
                    macro_rules! parse_helper {
                        ($n:literal) => {
                            draw_data.update_vertices_array(
                                data.into_iter().filter_map(|val| {
                                    let map = val.try_cast::<rhai::Map>()?;
                                    let mut out = [0u8; $n];
                                    parser(map, &mut out)?;
                                    Some(out)
                                }),
                            )
                        };
                    }

                    let result = match def_name.as_str() {
                        "line-rgb" => parse_helper!(40),
                        "rect-rgb" => parse_helper!(32),
                        "path-slot" => parse_helper!(20),
                        e => {
                            return Err(format!(
                                "unknown sublayer definition: `{}`",
                                e
                            )
                            .into());
                        }
                    };

                    if let Err(e) = result {
                        return Err(
                            format!("sublayer update error: {:?}", e).into()
                        );
                    } else {
                        return Ok(());
                    }
                }
            }

            Ok(())
        },
    );

    let layers = compositor.layers.clone();

    module.set_native_fn(
        "update_sublayer",
        move |layer_name: &str,
              sublayer_name: &str,
              vertex_data: rhai::Array,
              sets: rhai::Array| {
            let mut layers = layers.write();

            if let Some(layer) = layers.get_mut(layer_name) {
                if let Some(sublayer) = layer.get_sublayer_mut(sublayer_name) {
                    let parser =
                        sublayer.def.parse_rhai_vertex.clone().unwrap();

                    let def_name = sublayer.def_name.clone();
                    // TODO this only updates the first draw data set for now
                    let draw_data = sublayer.draw_data_mut().next().unwrap();

                    // need a macro since arrays have fixed length
                    macro_rules! parse_helper {
                        ($n:literal) => {{
                            draw_data.update_sets(
                                sets.into_iter().filter_map(|val| {
                                    val.try_cast::<DescSetIx>()
                                }),
                            );

                            draw_data.update_vertices_array(
                                vertex_data.into_iter().filter_map(|val| {
                                    let map = val.try_cast::<rhai::Map>()?;
                                    let mut out = [0u8; $n];
                                    parser(map, &mut out)?;
                                    Some(out)
                                }),
                            )
                        }};
                    }

                    let result = match def_name.as_str() {
                        "line-rgb" => parse_helper!(40),
                        "rect-rgb" => parse_helper!(32),
                        "path-slot" => parse_helper!(24),
                        e => {
                            return Err(format!(
                                "unknown sublayer definition: `{}`",
                                e
                            )
                            .into());
                        }
                    };

                    if let Err(e) = result {
                        return Err(
                            format!("sublayer update error: {:?}", e).into()
                        );
                    } else {
                        return Ok(());
                    }
                }
            }

            Ok(())
        },
    );

    module
}

pub mod slot {

    use super::*;

    pub fn vertex_buffer(
        engine: &mut VkEngine,
        capacity: usize,
    ) -> Result<BufferIx> {
        engine.with_allocators(|ctx, res, alloc| {
            let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;
            let usage = vk::BufferUsageFlags::VERTEX_BUFFER;
            // | vk::BufferUsageFlags::TRANSFER_SRC
            // | vk::BufferUsageFlags::TRANSFER_DST;
            let mut buffer = res.allocate_buffer(
                ctx,
                alloc,
                mem_loc,
                20,
                capacity,
                usage,
                Some("slot vertex buffer"),
            )?;

            let buf_ix = res.insert_buffer(buffer);
            Ok(buf_ix)
        })
    }

    pub fn sublayer(
        ctx: &VkContext,
        res: &mut GpuResources,
        clear_pass: vk::RenderPass,
        load_pass: vk::RenderPass,
    ) -> Result<SublayerDef> {
        let vert = res.load_shader(
            "shaders/path_slot_indexed_tmp.vert.spv",
            vk::ShaderStageFlags::VERTEX,
        )?;
        let frag = res.load_shader(
            "shaders/path_slot_indexed_tmp.frag.spv",
            // vk::ShaderStageFlags::FRAGMENT,
            vk::ShaderStageFlags::VERTEX
                | vk::ShaderStageFlags::COMPUTE
                | vk::ShaderStageFlags::FRAGMENT,
        )?;

        let vert = res.insert_shader(vert);
        let frag = res.insert_shader(frag);

        let vertex_stride = std::mem::size_of::<[f32; 6]>();

        let vert_binding_desc = vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(vertex_stride as u32)
            .input_rate(vk::VertexInputRate::INSTANCE)
            .build();

        let pos_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(0)
            .build();

        let size_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(8)
            .build();

        let ix_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(2)
            .format(vk::Format::R32G32_UINT)
            .offset(16)
            .build();

        let vert_binding_descs = [vert_binding_desc];
        let vert_attr_descs = [pos_desc, size_desc, ix_desc];

        let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&vert_binding_descs)
            .vertex_attribute_descriptions(&vert_attr_descs);

        let vertex_offset = 0;

        let mut def = SublayerDef::new::<([f32; 2], [f32; 2], [u32; 2]), _>(
            ctx,
            res,
            "path-slot",
            vert,
            frag,
            clear_pass,
            load_pass,
            vertex_offset,
            vertex_stride,
            true,
            Some(6),
            None,
            vert_input_info,
            None,
            // [font_desc_set],
            None,
        )?;

        fn get_cast(map: &rhai::Map, k: &str) -> Option<f32> {
            let field = map.get(k)?;
            field
                .as_float()
                .ok()
                .or(field.as_int().ok().map(|v| v as f32))
        }

        def.set_parser(|map, out| {
            let x = get_cast(&map, "x")?;
            let y = get_cast(&map, "y")?;
            let w = get_cast(&map, "w")?;
            let h = get_cast(&map, "h")?;

            let o = map.get("o").and_then(|f| f.as_int().ok())?;
            let l = map.get("l").and_then(|f| f.as_int().ok())?;

            out[0..8].clone_from_slice([x, y].as_bytes());
            out[8..16].clone_from_slice([w, h].as_bytes());
            out[16..24].clone_from_slice([o as u32, l as u32].as_bytes());
            Some(())
        });

        Ok(def)
    }
}
