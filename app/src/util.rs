use wgpu::util::DeviceExt;

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

pub struct Uniform<T, const N: usize> {
    pub name: String,
    data: T,

    buffer: wgpu::Buffer,

    to_bytes: Box<dyn Fn(&T) -> [u8; N]>,
    // need_write: bool,
}

impl<T, const N: usize> Uniform<T, N> {
    pub fn new(
        state: &raving_wgpu::State,
        usage: wgpu::BufferUsages,
        name: &str,
        data: T,
        to_bytes: impl Fn(&T) -> [u8; N] + 'static,
    ) -> anyhow::Result<Self> {
        if (N % wgpu::COPY_BUFFER_ALIGNMENT as usize) != 0 {
            let al = wgpu::COPY_BUFFER_ALIGNMENT as usize;
            anyhow::bail!("Uniform buffer size must be divisible by {al}, was {N}; {N} % {al} = {}",
                          N % al);
        }

        let to_bytes = Box::new(to_bytes);
        let name = name.to_string();

        let buf_data = to_bytes(&data);

        let label_str = format!("Uniform: {name}");

        let buffer = state.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some(&label_str),
                contents: bytemuck::cast_slice(&buf_data),
                usage,
            },
        );

        Ok(Self {
            name,
            data,
            buffer,
            to_bytes,
            // need_write: false,
        })
    }

    pub fn data_ref(&self) -> &T {
        &self.data
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub fn buffer_size(&self) -> usize {
        N
    }

    pub fn update_data(&mut self, f: impl FnOnce(&mut T)) {
        f(&mut self.data);
        // self.need_write = true;
    }

    // pub fn update_data_maybe_write(&mut self, f: impl FnOnce(&mut T) -> bool) {
    //     let w = f(&mut self.data);
    // self.need_write = w;
    // }

    pub fn write_buffer(&mut self, state: &raving_wgpu::State) {
        let data = (self.to_bytes)(&self.data);
        state.queue.write_buffer(&self.buffer, 0, data.as_slice());
    }

    //
}
