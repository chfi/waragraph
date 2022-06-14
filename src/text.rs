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
    SamplerIx, VkEngine,
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
    glyph_vertices: Vec<GlyphVx>,

    pub cache_img: ImageIx,
    pub cache_img_view: ImageViewIx,
    pub cache_texture_set: DescSetIx,

    layout_info: DescriptorLayoutInfo,
    set_info: BTreeMap<u32, DescriptorInfo>,

    sampler: SamplerIx,
}

impl TextCache {
    pub fn cache_format() -> vk::Format {
        vk::Format::R8_UNORM
    }

    fn allocate_cache_data(
        engine: &mut VkEngine,
        layout_info: &DescriptorLayoutInfo,
        set_info: &BTreeMap<u32, DescriptorInfo>,
        sampler: vk::Sampler,
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

                    let sampler_info = vk::DescriptorImageInfo::builder()
                        .sampler(sampler)
                        .build();

                    desc_builder.bind_image(0, &[sampler_info]);
                    desc_builder.bind_image(1, &[info]);

                    Ok(())
                },
            )?;

            let view_ix = res.insert_image_view(image_view);
            let set_ix = res.insert_desc_set(desc_set);

            Ok((image_ix, view_ix, set_ix))
        })?;

        engine.submit_queue_fn(|ctx, res, alloc, cmd| {
            let src_access_mask = vk::AccessFlags::empty();
            let src_stage_mask = vk::PipelineStageFlags::TOP_OF_PIPE;

            let dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            let dst_stage_mask = vk::PipelineStageFlags::TRANSFER;

            let (image_ix, _, _) = result;
            let image = &res[image_ix];

            VkEngine::transition_image(
                cmd,
                ctx.device(),
                image.image,
                src_access_mask,
                src_stage_mask,
                dst_access_mask,
                dst_stage_mask,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );

            Ok(())
        })?;

        Ok(result)
    }

    pub fn upload_data(&mut self, engine: &mut VkEngine) -> Result<()> {
        log::warn!("uploading glyph data");

        let staging = engine.submit_queue_fn(|ctx, res, alloc, cmd| {
            let image = &mut res[self.cache_img];

            // transition image to TRANSFER_DST_OPTIMAL

            let src_access_mask = vk::AccessFlags::empty();
            let src_stage_mask = vk::PipelineStageFlags::TOP_OF_PIPE;

            let dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            let dst_stage_mask = vk::PipelineStageFlags::TRANSFER;

            VkEngine::transition_image(
                cmd,
                ctx.device(),
                image.image,
                src_access_mask,
                src_stage_mask,
                dst_access_mask,
                dst_stage_mask,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );

            let layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
            let staging = image.fill_from_pixels(
                ctx.device(),
                ctx,
                alloc,
                self.cache_data.iter().copied(),
                1,
                layout,
                cmd,
            )?;

            // transition image back to SHADER_READ_ONLY_OPTIMAL

            let src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            let src_stage_mask = vk::PipelineStageFlags::TRANSFER;

            let dst_access_mask = vk::AccessFlags::SHADER_READ;
            let dst_stage_mask = vk::PipelineStageFlags::FRAGMENT_SHADER;

            VkEngine::transition_image(
                cmd,
                ctx.device(),
                image.image,
                src_access_mask,
                src_stage_mask,
                dst_access_mask,
                dst_stage_mask,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );

            // let tex_dims = self.brush.texture_dimensions();

            // let image = {};
            // let staging =

            // if needed, recreate the image (and update the descriptor set)

            //
            Ok(staging)
        })?;

        engine.resources.free_buffer(
            &engine.context,
            &mut engine.allocator,
            staging,
        )?;

        Ok(())
    }

    pub fn reallocate(
        &mut self,
        engine: &mut VkEngine,
        width: u32,
        height: u32,
    ) -> Result<()> {
        log::error!("reallocating to ({}, {})", width, height);
        let format = Self::cache_format();

        engine.with_allocators(|ctx, res, alloc| {
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

            if let Some(old_view) =
                res.insert_image_view_at(self.cache_img_view, image_view)
            {
                unsafe {
                    ctx.device().destroy_image_view(old_view, None);
                };
            }

            if let Some(old_image) =
                res.insert_image_at(self.cache_img, image_res)
            {
                res.free_image(ctx, alloc, old_image)?;
            }

            let layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

            res.write_desc_set_raw(
                &self.set_info,
                res[self.cache_texture_set],
                |res, desc_builder| {
                    let info = vk::DescriptorImageInfo::builder()
                        .image_layout(layout)
                        .image_view(image_view)
                        .build();

                    let sampler_info = vk::DescriptorImageInfo::builder()
                        .sampler(res[self.sampler])
                        .build();

                    desc_builder.bind_image(0, &[sampler_info]);
                    desc_builder.bind_image(1, &[info]);

                    Ok(())
                },
            )?;

            Ok(())
        })?;

        self.brush.resize_texture(width, height);

        engine.submit_queue_fn(|ctx, res, alloc, cmd| {
            let src_access_mask = vk::AccessFlags::empty();
            let src_stage_mask = vk::PipelineStageFlags::TOP_OF_PIPE;

            let dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            let dst_stage_mask = vk::PipelineStageFlags::TRANSFER;

            let image = &res[self.cache_img];

            VkEngine::transition_image(
                cmd,
                ctx.device(),
                image.image,
                src_access_mask,
                src_stage_mask,
                dst_access_mask,
                dst_stage_mask,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );

            Ok(())
        })?;

        Ok(())
    }

    pub fn update_layer(
        &self,
        compositor: &mut Compositor,
        layer_name: &str,
        sublayer_name: &str,
    ) -> Result<()> {
        compositor.with_layer(layer_name, |layer| {
            if let Some(sublayer_data) = layer
                .get_sublayer_mut(sublayer_name)
                .and_then(|sub| sub.draw_data_mut().next())
            {
                sublayer_data.update_vertices_array(
                    self.glyph_vertices.iter().copied(),
                )?;

                let desc_set = self.cache_texture_set;
                let new_sets = [desc_set];
                sublayer_data.update_sets(new_sets);
            }

            Ok(())
        })?;

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
        let _dejavu = FontArc::try_from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/dejavu-fonts-ttf-2.37/ttf/DejaVuSans.ttf"
        )))?;

        let dejavu = FontArc::try_from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/dejavu-fonts-ttf-2.37/ttf/DejaVuSerif.ttf"
        )))?;

        let dejavu_mono = FontArc::try_from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/dejavu-fonts-ttf-2.37/ttf/DejaVuSansMono.ttf"
        )))?;

        let dejavu_bold = FontArc::try_from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/dejavu-fonts-ttf-2.37/ttf/DejaVuSansMono-Bold.ttf"
        )))?;

        let mut glyph_brush: GlyphBrushBuilder<_> =
            GlyphBrushBuilder::using_font(dejavu);
        // .draw_cache_position_tolerance(0.5)
        // .draw_cache_scale_tolerance(1.0)
        //
        // .draw_cache_position_tolerance(0.0)
        // .draw_cache_scale_tolerance(0.0)
        //
        // .draw_cache_scale_tolerance(1000.0)
        // .build();

        /*
        {
            let mut builder = glyph_brush.draw_cache_builder.clone();
            builder = builder.pad_glyphs(true);
            glyph_brush.draw_cache_builder = builder;
        }
        */

        let mut glyph_brush: GlyphBrush<GlyphVx> =
            glyph_brush.initial_cache_size((16, 16)).build();
        // GlyphBrushBuilder::using_font(dejavu_bold).build();

        let (width, height) = glyph_brush.texture_dimensions();

        let capacity = (width * height) as usize;

        let cache_data = vec![0u8; capacity];

        log::error!("glyph_brush texture dimensions: {:?}", (width, height));

        let sampler = {
            let norm_sampler_info = vk::SamplerCreateInfo::builder()
                // .mag_filter(vk::Filter::LINEAR)
                // .min_filter(vk::Filter::LINEAR)
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                // .address_mode_u(vk::SamplerAddressMode::REPEAT)
                // .address_mode_v(vk::SamplerAddressMode::REPEAT)
                // .address_mode_w(vk::SamplerAddressMode::REPEAT)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                // .anisotropy_enable(false)
                .anisotropy_enable(true)
                .max_anisotropy(16.0)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                .mip_lod_bias(0.0)
                .min_lod(0.0)
                .max_lod(1.0)
                .unnormalized_coordinates(false)
                .build();

            engine
                .resources
                .insert_sampler(&engine.context, norm_sampler_info)?
        };

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

        let (cache_img, cache_img_view, cache_texture_set) =
            Self::allocate_cache_data(
                engine,
                &layout_info,
                &set_info,
                engine.resources[sampler],
                Self::cache_format(),
                width,
                height,
            )?;

        Ok(Self {
            brush: glyph_brush,
            cache_data,
            glyph_vertices: Vec::new(),

            cache_img,
            cache_img_view,
            cache_texture_set,

            layout_info,
            set_info,

            sampler,
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

    pub fn process_queued(
        &mut self,
        engine: &mut VkEngine,
        compositor: &mut Compositor,
    ) -> Result<()> {
        log::error!("processing queued sections");

        // let mut glyphs_to_upload = Vec::new();

        let result = self.brush.process_queued(
            |rect, tex_data| {
                log::warn!("received rect: {:?}", rect);
                log::warn!("tex_data len: {}", tex_data.len());

                let [tex_x0, tex_y0] = rect.min;
                let tex_w = rect.width();
                let tex_h = rect.height();

                for row_i in 0..tex_h {
                    let data_row_ix = (tex_w * row_i) as usize;
                    let data_row_end = data_row_ix + tex_w as usize;
                    let row = &tex_data[data_row_ix..data_row_end];

                    let tgt_row = (tex_y0 + row_i) as usize;
                    let tgt_col = tex_x0 as usize;
                    let width = tex_w as usize;
                    let tgt_row_ix = width * tgt_row + tgt_col;
                    let tgt_row_end = tgt_row_ix + tex_w as usize;

                    self.cache_data[tgt_row_ix..tgt_row_end]
                        .clone_from_slice(row);
                }
            },
            |glyph_vx| {
                let tex = glyph_vx.tex_coords;
                let pixel = glyph_vx.pixel_coords;

                let dst_p: ScreenPoint = point2(pixel.min.x, pixel.min.y);
                let src_p: ScreenPoint = point2(tex.min.x, tex.min.y);

                let dst_s: ScreenSize = size2(pixel.width(), pixel.height());
                let src_s: ScreenSize = size2(tex.width(), tex.height());

                let dst = ScreenRect {
                    origin: dst_p,
                    size: dst_s,
                };
                let src = ScreenRect {
                    origin: src_p,
                    size: src_s,
                };

                let color = rgb::RGBA::new(0.0f32, 0.0, 0.0, 1.0);

                // log::warn!("processing glyph: {:?}", glyph_vx);

                glyph_vertex(dst, src, color)
            },
        );

        match result {
            Ok(BrushAction::Draw(vertices)) => {
                log::warn!("updating glyphs with {} vertices", vertices.len());
                self.glyph_vertices = vertices;
            }
            Ok(BrushAction::ReDraw) => {
                log::warn!("redraw glyphs")
            }
            Err(BrushError::TextureTooSmall { suggested }) => {
                let (x, y) = suggested;
                let capacity = (x * y) as usize;
                self.cache_data.resize(capacity, 0u8);
                self.glyph_vertices.clear();
                log::warn!("reallocating glyph cache");

                self.reallocate(engine, x, y)?;

                log::warn!("trying again");
                self.process_queued(engine, compositor)?;
            }
        }

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
