#[derive(Debug)]
pub struct BufferDesc {
    pub buffer: wgpu::Buffer,
    pub size: usize,
}

impl BufferDesc {
    pub fn new(buffer: wgpu::Buffer, size: usize) -> Self {
        Self { buffer, size }
    }
}
