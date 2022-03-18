use std::collections::BTreeMap;

use ash::vk;
use bstr::ByteSlice;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    context::VkContext, descriptor::DescriptorLayoutInfo, BufferIx, BufferRes,
    DescSetIx, GpuResources,
};
use rspirv_reflect::DescriptorInfo;

use zerocopy::{AsBytes, FromBytes};

use anyhow::{anyhow, Result};

use crossbeam::atomic::AtomicCell;
use std::sync::Arc;

use crate::{graph::Waragraph, util::LabelStorage};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SamplerMode {
    Point,
    Edges,
    Range,
}

// P -- the original domain, linear domain (e.g. pangenome position
// space, or a sequence)

// T -- the "output" sample type, the sampler essentially maps P to
// collections of T, which can then be reduced to a single sample
// value
pub trait Sampler<P, T> {
    // fn sample_range(
    // type Sample
}

// pangenome pos -> (Node, Offset)

// pub struct PointSampler<'a> {
pub struct PointSampler<S, P>
where
    S: AsRef<[P]>,
    P: Copy,
{
    source: S,
    _value: std::marker::PhantomData<P>,
}
