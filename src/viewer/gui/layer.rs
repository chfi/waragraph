use crossbeam::atomic::AtomicCell;
use parking_lot::RwLock;
use raving::compositor::label_space::LabelSpace;
use raving::vk::context::VkContext;
use raving::vk::{
    BufferIx, DescSetIx, FramebufferIx, GpuResources, PipelineIx, RenderPassIx,
    ShaderIx, VkEngine,
};

use raving::compositor::*;
use raving::vk::resource::WindowResources;

use ash::{vk, Device};

use rhai::plugin::RhaiResult;
use rustc_hash::{FxHashMap, FxHashSet};
use winit::event::VirtualKeyCode;
use winit::window::Window;

use crate::config::ConfigMap;
use crate::console::{RhaiBatchFn2, RhaiBatchFn4, RhaiBatchFn5};
use crate::graph::{Node, Waragraph};
use crate::util::{BufFmt, BufId, BufferStorage, LabelStorage};
use crate::viewer::{SlotRenderers, ViewDiscrete1D};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

use rhai::plugin::*;

pub fn add_sublayer_defs(
    engine: &mut VkEngine,
    compositor: &mut Compositor,
    font_desc_set: DescSetIx,
) -> Result<()> {
    engine.with_allocators(|ctx, res, _| {
        let pass = res[compositor.pass];
        compositor.add_sublayer_defs([
            text_sublayer(ctx, res, font_desc_set, pass)?,
            rect_palette_sublayer(ctx, res, pass)?,
            rect_rgb_sublayer(ctx, res, pass)?,
            line_rgb_sublayer(ctx, res, pass)?,
        ]);

        Ok(())
    })
}

pub(super) fn rect_palette_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    pass: vk::RenderPass,
) -> Result<SublayerDef> {
    let vert = res.load_shader(
        "shaders/tri_2d_window.vert.spv",
        vk::ShaderStageFlags::VERTEX
            | vk::ShaderStageFlags::FRAGMENT
            | vk::ShaderStageFlags::COMPUTE,
    )?;
    let frag = res.load_shader(
        "shaders/rect_flat_color.frag.spv",
        vk::ShaderStageFlags::FRAGMENT,
    )?;

    let vert = res.insert_shader(vert);
    let frag = res.insert_shader(frag);

    let vert_binding_desc = vk::VertexInputBindingDescription::builder()
        .binding(0)
        .stride(std::mem::size_of::<[f32; 3]>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX)
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
        .format(vk::Format::R32_UINT)
        .offset(8)
        .build();

    let vert_binding_descs = [vert_binding_desc];
    let vert_attr_descs = [pos_desc, ix_desc];

    let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&vert_binding_descs)
        .vertex_attribute_descriptions(&vert_attr_descs);

    let vertex_offset = 12;
    let vertex_stride = 12;

    // VkEngine::set_debug_object_name(

    SublayerDef::new::<([f32; 4], u32), _>(
        ctx,
        res,
        "rect-palette",
        vert,
        frag,
        pass,
        vertex_offset,
        vertex_stride,
        false,
        None,
        Some(1),
        vert_input_info,
        None,
    )
}

pub(super) fn text_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    font_desc_set: DescSetIx,
    pass: vk::RenderPass,
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
        pass,
        vertex_offset,
        vertex_stride,
        true,
        Some(6),
        None,
        vert_input_info,
        [font_desc_set],
    )
}

pub(super) fn rect_rgb_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    pass: vk::RenderPass,
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

    SublayerDef::new::<([f32; 2], [f32; 2], [f32; 4]), _>(
        ctx,
        res,
        "rect-rgb",
        vert,
        frag,
        pass,
        vertex_offset,
        vertex_stride,
        true,
        Some(6),
        None,
        vert_input_info,
        None,
    )
}

pub(super) fn line_rgb_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    pass: vk::RenderPass,
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

    SublayerDef::new::<([f32; 3], [f32; 3], [f32; 4]), _>(
        ctx,
        res,
        "line-rgb",
        vert,
        frag,
        pass,
        vertex_offset,
        vertex_stride,
        true,
        Some(6),
        None,
        vert_input_info,
        None,
    )
}

pub fn create_rhai_module(compositor: &Compositor) -> rhai::Module {
    let mut module = raving::compositor::create_rhai_module(compositor);

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
                                sublayer.update_vertices_array(vertices);

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

            let get_cast = |map: &rhai::Map, k: &str| {
                let field = map.get(k)?;
                field
                    .as_float()
                    .ok()
                    .or(field.as_int().ok().map(|v| v as f32))
            };

            if let Some(layer) = layers.get_mut(layer_name) {
                if let Some(sublayer) = layer.get_sublayer_mut(sublayer_name) {
                    // let def_name = sublayer.def_name.clone();
                    match sublayer.def_name.as_str() {
                        "rect-rgb" => {
                            let result = sublayer.update_vertices_array(
                                data.into_iter().filter_map(|val| {
                                    let map = val.try_cast::<rhai::Map>()?;

                                    let mut out = [0u8; 32];

                                    let x = get_cast(&map, "x")?;
                                    let y = get_cast(&map, "y")?;
                                    let w = get_cast(&map, "w")?;
                                    let h = get_cast(&map, "h")?;

                                    let r = get_cast(&map, "r")?;
                                    let g = get_cast(&map, "g")?;
                                    let b = get_cast(&map, "b")?;
                                    let a = get_cast(&map, "a")?;

                                    out[0..8]
                                        .clone_from_slice([x, y].as_bytes());
                                    out[8..16]
                                        .clone_from_slice([w, h].as_bytes());
                                    out[16..32].clone_from_slice(
                                        [r, g, b, a].as_bytes(),
                                    );

                                    Some(out)
                                }),
                            );

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
                                "unknown sublayer definition: `{}`",
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

    module
}

#[export_module]
pub mod rhai_module {

    pub type Layer = super::Layer;
    pub type Sublayer = super::Sublayer;
}
