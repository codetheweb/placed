use std::{cmp::min, mem::size_of, num::NonZeroU32};

use archive::structures::{Meta, StoredTilePlacement};
use wgpu::util::DeviceExt;

pub struct TextureUpdateByCoords {
    texture: wgpu::Texture,
    texture_extent: wgpu::Extent3d,
    pub texture_view: wgpu::TextureView,
    input_buffer: wgpu::Buffer,
    compute_pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
}

impl TextureUpdateByCoords {
    pub fn new(device: &wgpu::Device, meta: Meta) -> Self {
        let shader = wgpu::include_wgsl!("../shaders/texture_update_by_coords.compute.wgsl");
        let module = device.create_shader_module(shader);

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &module,
            entry_point: "main",
        });

        // todo: pull struct size automatically
        let MAX_SIZE = (size_of::<u32>() * 5)
            * (device.limits().max_compute_workgroups_per_dimension as usize);

        let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("texture_update_by_coords input buffer"),
            contents: bytemuck::cast_slice(&vec![0; MAX_SIZE]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let mut r: Vec<[u32; 4]> = meta
            .clone()
            .color_id_to_tuple
            .into_values()
            .map(|x| [x[0] as u32, x[1] as u32, x[2] as u32, x[3] as u32])
            .collect();

        // Pad to 256 color tuples
        while r.len() < 256 {
            r.push([0, 0, 0, 0]);
        }

        let locals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("texture_update_by_coords locals buffer"),
            contents: bytemuck::cast_slice(&r),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let size = meta.get_largest_canvas_size().unwrap();
        let texture_extent = wgpu::Extent3d {
            width: size.width.into(),
            height: size.height.into(),
            depth_or_array_layers: 1,
        };

        let texture_desc = wgpu::TextureDescriptor {
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                // temp
                | wgpu::TextureUsages::COPY_SRC,
            label: None,
        };
        let texture = device.create_texture(&texture_desc);

        let some_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            base_mip_level: 0,
            mip_level_count: NonZeroU32::new(1),
            ..Default::default()
        });

        let bind_group_layout = compute_pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: locals_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&some_view),
                },
            ],
        });

        Self {
            input_buffer,
            texture,
            texture_extent,
            texture_view: some_view,
            compute_pipeline,
            bind_group,
        }
    }

    /// Make sure to only pass one tile per position, as it's not guaranteed that the order of tiles will be preserved during rendering.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        chunk: Vec<u8>,
    ) {
        let num_of_tiles = chunk.len() / StoredTilePlacement::encoded_size();

        let limit = device.limits().max_compute_workgroups_per_dimension / 4;

        let mut i = 0;
        while i < num_of_tiles {
            let current = &chunk[i..min(i + (limit as usize) + 1, chunk.len())];

            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("texture_update_by_coords compute pass"),
                });
                cpass.set_pipeline(&self.compute_pipeline);
                cpass.set_bind_group(0, &self.bind_group, &[]);
                cpass.dispatch_workgroups(current.len() as u32, 1, 1);
            }

            let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&current),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });

            encoder.copy_buffer_to_buffer(
                &staging_buffer,
                0,
                &self.input_buffer,
                0,
                (current.len()) as u64,
            );

            i = i + (device.limits().max_compute_workgroups_per_dimension) as usize + 1;
        }
    }
}

struct TestHelpers {}

impl TestHelpers {
    pub fn get_device() -> (wgpu::Device, wgpu::Queue) {
        pollster::block_on(Self::get_device_async())
    }
    async fn get_device_async() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::Backends::all());

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, num::NonZeroU32, sync::mpsc};

    use archive::structures::{CanvasSizeChange, Meta, StoredTilePlacement};

    use super::{TestHelpers, TextureUpdateByCoords};

    #[test]
    fn smoke() {
        let mut color_id_to_tuple = HashMap::new();
        color_id_to_tuple.insert(0, [0, 0, 0, 1]);

        let texture_size: u32 = 64;

        let meta = Meta {
            chunk_descs: vec![],
            color_id_to_tuple,
            last_pixel_placed_at_seconds_since_epoch: 0,
            canvas_size_changes: vec![CanvasSizeChange {
                width: texture_size as u16,
                height: texture_size as u16,
                ms_since_epoch: 0,
            }],
        };

        let mut data: Vec<u8> = Vec::new();

        // Fill with black
        for x in 0..texture_size {
            for y in 0..texture_size {
                let tile = StoredTilePlacement {
                    x: x as u16,
                    y: y as u16,
                    color_index: 0,
                    ms_since_epoch: 0,
                };

                tile.write_into(&mut data);
            }
        }

        let (device, queue) = TestHelpers::get_device();

        let mut updater = TextureUpdateByCoords::new(&device, meta);

        let u32_size = std::mem::size_of::<u32>() as u32;

        let output_buffer_size = (u32_size * texture_size * texture_size) as wgpu::BufferAddress;
        let output_buffer_desc = wgpu::BufferDescriptor {
            size: output_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST
        // this tells wpgu that we want to read this buffer from the cpu
        | wgpu::BufferUsages::MAP_READ,
            label: None,
            mapped_at_creation: false,
        };
        let output_buffer = device.create_buffer(&output_buffer_desc);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("texture_update_by_coords encoder"),
        });

        updater.update(&device, &mut encoder, data);

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &updater.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new(u32_size * texture_size),
                    rows_per_image: NonZeroU32::new(texture_size),
                },
            },
            updater.texture_extent,
        );

        queue.submit(Some(encoder.finish()));

        // Read texture
        {
            let buffer_slice = output_buffer.slice(..);

            // NOTE: We have to create the mapping THEN device.poll() before await
            // the future. Otherwise the application will freeze.
            let (tx, rx) = mpsc::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).unwrap();
            });
            device.poll(wgpu::Maintain::Wait);
            rx.recv().unwrap().unwrap();

            let data = buffer_slice.get_mapped_range();

            use image::{ImageBuffer, Rgba};
            let buffer =
                ImageBuffer::<Rgba<u8>, _>::from_raw(texture_size, texture_size, data).unwrap();
            println!("buffer: {:?}", buffer);
            buffer.save("image.png").unwrap();
        }
        output_buffer.unmap();
    }
}
