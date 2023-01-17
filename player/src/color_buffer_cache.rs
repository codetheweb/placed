use std::collections::HashMap;

use wgpu::{util::DeviceExt, Buffer, BufferUsages, Device};

pub struct ColorBufferCache {
    cache: HashMap<[u8; 4], Buffer>,
}

impl ColorBufferCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get(&mut self, color: &[u8; 4], device: &Device) -> &Buffer {
        self.cache.entry(*color).or_insert_with(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: color,
                usage: BufferUsages::COPY_SRC,
            })
        })
    }
}
