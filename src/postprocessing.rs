use ash::vk;
use raving::vk::{
    BufferIx, DescSetIx, FramebufferIx, GpuResources, ImageIx,
    ImageViewIx, PipelineIx, RenderPassIx, ShaderIx, VkContext, VkEngine,
};

use anyhow::Result;

#[derive(Clone, Copy)]
pub struct PostprocessDef {
    pass: RenderPassIx,
    pipeline: PipelineIx,
}

static POSTPROCESS_VERT_SPIRV_SRC: &'static [u8] =
    raving::include_shader!("postprocessing/base.vert.spv");

impl PostprocessDef {
    pub fn new(engine: &mut VkEngine, frag: ShaderIx) -> Result<Self> {
        todo!();
    }
}

pub struct PostprocessEffect {
    def: PostprocessDef,

    sets: Vec<DescSetIx>,
}
