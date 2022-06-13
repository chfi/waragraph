//! Glyph cache and text rendering utilities

use std::borrow::Cow;
use std::collections::BTreeMap;

use euclid::{point2, size2};
use glyph_brush::ab_glyph::FontArc;
// use parking_lot::RwLock;
// use raving::compositor::label_space::LabelSpace;
use raving::vk::context::VkContext;
use raving::vk::descriptor::DescriptorLayoutInfo;
use raving::vk::{
    BufferIx, DescSetIx, GpuResources, ImageIx, ImageViewIx, PipelineIx,
    VkEngine,
};

use raving::compositor::*;

use ash::vk;
use rspirv_reflect::DescriptorInfo;

use crate::geometry::{ScreenPoint, ScreenRect, ScreenSize};

use anyhow::Result;

use glyph_brush::*;

pub type GlyphVx = [u8; 48];

pub struct CacheData {}

// impl CacheData {
//     fn new
// }

pub struct TextCache {
    pub brush: GlyphBrush<GlyphVx>,

    cache_data: Vec<u8>,

    /*
    pub cache_img: ImageIx,
    pub cache_img_view: ImageViewIx,
    pub cache_texture_set: DescSetIx,
    */
    layout_info: DescriptorLayoutInfo,
    set_info: BTreeMap<u32, DescriptorInfo>,
    // sampler: SamplerIx,
}

impl TextCache {
    pub fn cache_format() -> vk::Format {
        vk::Format::R8_UNORM
    }

    /*
    fn allocate_cache_data(
        engine: &mut VkEngine,
        layout_info: &DescriptorLayoutInfo,
        set_info: &BTreeMap<u32, DescriptorInfo>,
        format: vk::Format,
        width: u32,
        height: u32,
    ) -> Result<(ImageIx, ImageViewIx, DescSetIx)> {
        let result = engine.with_allocators(|ctx, res, alloc| {
            let usage = vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::STORAGE;

            let name = format!("TextCache:{},{}:image", width, height);

            let image_res = res.allocate_image(
                ctx,
                alloc,
                width,
                height,
                format,
                usage,
                Some(&name),
            )?;

            let image_view = image_res.create_image_view(ctx)?;

            let image_ix = res.insert_image(image_res);

            let layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

            let desc_set = res.allocate_desc_set_raw(
                layout_info,
                set_info,
                |res, desc_builder| {
                    let info = vk::DescriptorImageInfo::builder()
                        .image_layout(layout)
                        .image_view(image_view)
                        .build();

                    desc_builder.bind_image(0, &[info]);

                    Ok(())
                },
            )?;

            //

            //
        })?;

        Ok(result)
    }
    */

    fn upload_cache(&self, engine: &mut VkEngine, data: &[u8]) -> Result<()> {
        Ok(())
    }

    fn insert_row(&mut self, x: usize, y: usize, pixels: &[u8]) {
        let (cols, rows) = self.brush.texture_dimensions();

        let w = cols as usize;
        let h = rows as usize;

        let offset = y * h + x;

        let len = pixels.len();

        assert!(
            x + len < w,
            "Tried to insert pixel data that would extend to the next row"
        );

        self.cache_data[offset..offset + len].clone_from_slice(pixels);
    }

    pub fn new(engine: &mut VkEngine, compositor: &Compositor) -> Result<Self> {
        let dejavu = FontArc::try_from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/dejavu-fonts-ttf-2.37/ttf/DejaVuSansMono.ttf"
        )))?;

        let glyph_brush: GlyphBrush<GlyphVx> =
            GlyphBrushBuilder::using_font(dejavu).build();

        let (width, height) = glyph_brush.texture_dimensions();

        let capacity = width * height;

        let cache_data = vec![0u8; capacity];

        log::error!("glyph_brush texture dimensions: {:?}", (width, height));

        let (layout_info, set_info) = {
            let sublayer_def = compositor.sublayer_defs.get("glyph").expect("error: `glyph` sublayer not found, has the sublayer definition been added?");

            let pipeline = &engine.resources[sublayer_def.load_pipeline];

            let shader_ix = pipeline
                .fragment
                .expect("`glyph` sublayer is missing fragment shader");

            let shader = &engine.resources[shader_ix];

            let set_ix = 0;

            let set_info = shader.set_infos.get(&set_ix).unwrap().clone();
            let layout_info = shader.set_layout_info(set_ix)?;

            (layout_info, set_info)
        };

        /*
        let (cache_img, cache_img_view, cache_texture_set) =
            Self::allocate_cache_data(
                engine,
                &layout_info,
                &set_info,
                Self::cache_format(),
                width,
                height,
            )?;
        */

        Ok(Self {
            brush: glyph_brush,

            // cache_img,
            // cache_img_view,
            // cache_texture_set,
            layout_info,
            set_info,
        })
    }

    pub fn queue<'a, S>(
        &mut self,
        // engine: &mut VkEngine,
        section: S,
    ) where
        S: Into<Cow<'a, Section<'a>>>,
    {
        self.brush.queue(section);
    }

    pub fn process_queued(&mut self, engine: &mut VkEngine) -> Result<()> {
        log::error!("processing queued sections");

        let mut glyphs_to_upload = Vec::new();

        self.brush.process_queued(
            |rect, tex_data| {
                //
            },
            |glyph_vx| {
                //
            },
        )?;

        /*
        engine.submit_queue_fn(|ctx, res, alloc, cmd| {
            let tex_dims = self.brush.texture_dimensions();

            // let image = {};

            // create a staging buffer
            // fill it with the pixel data from self.cache_data

            // if needed, recreate the image (and update the descriptor set)

            //
        })?;
        */

        log::warn!("results: {:?}", result);

        Ok(())
    }
}

pub(crate) fn glyph_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    clear_pass: vk::RenderPass,
    load_pass: vk::RenderPass,
) -> Result<SublayerDef> {
    let vert = res
        .load_shader("shaders/glyph.vert.spv", vk::ShaderStageFlags::VERTEX)?;
    let frag = res.load_shader(
        "shaders/glyph.frag.spv",
        vk::ShaderStageFlags::FRAGMENT, // vk::ShaderStageFlags::VERTEX
                                        //     | vk::ShaderStageFlags::COMPUTE
                                        //     | vk::ShaderStageFlags::FRAGMENT,
    )?;

    let vert = res.insert_shader(vert);
    let frag = res.insert_shader(frag);

    type VertexShape = ([f32; 2], [f32; 2], [f32; 2], [f32; 2], [f32; 4]);

    let vertex_stride = std::mem::size_of::<VertexShape>();

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

    let uv_pos_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(2)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(16)
        .build();

    let uv_size_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(3)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(24)
        .build();

    let color_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(4)
        .format(vk::Format::R32G32B32A32_SFLOAT)
        .offset(32)
        .build();

    let vert_binding_descs = [vert_binding_desc];
    let vert_attr_descs =
        [pos_desc, size_desc, uv_pos_desc, uv_size_desc, color_desc];

    let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&vert_binding_descs)
        .vertex_attribute_descriptions(&vert_attr_descs);

    let vertex_offset = 0;

    SublayerDef::new::<VertexShape, _>(
        ctx,
        res,
        "glyph",
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
        [],
    )
}

fn glyph_vertex(
    dst: ScreenRect,
    src: ScreenRect,
    color: rgb::RGBA<f32>,
) -> GlyphVx {
    let mut out = [0u8; 48];

    let g_pos = [dst.origin.x, dst.origin.y];
    let g_size = [dst.size.width, dst.size.height];

    let uv_pos = [src.origin.x, src.origin.y];
    let uv_size = [src.size.width, src.size.height];

    out[0..8].clone_from_slice(bytemuck::cast_slice(&g_pos));
    out[8..16].clone_from_slice(bytemuck::cast_slice(&g_size));

    out[16..24].clone_from_slice(bytemuck::cast_slice(&uv_pos));
    out[24..32].clone_from_slice(bytemuck::cast_slice(&uv_size));

    out[32..48].clone_from_slice(bytemuck::cast_slice(&[
        color.r, color.g, color.b, color.a,
    ]));

    out
}
