use std::{mem::size_of, num::NonZeroU32};

use archive::structures::DecodedTilePlacement;
use wgpu::util::DeviceExt;

pub struct TextureUpdateByCoords {
    pub texture_view: wgpu::TextureView,
    input_buffer: wgpu::Buffer,
    compute_pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
}

const MAX_WORKGROUP_DISPATCH_SIZE: usize = 65535;

impl TextureUpdateByCoords {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = wgpu::include_wgsl!("../shaders/texture_update_by_coords.compute.wgsl");
        let module = device.create_shader_module(shader);

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &module,
            entry_point: "main",
        });

        // todo: pull struct size automatically
        let MAX_SIZE = (size_of::<u32>() * 5) * MAX_WORKGROUP_DISPATCH_SIZE;

        let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("texture_update_by_coords input buffer"),
            contents: bytemuck::cast_slice(&vec![0; MAX_SIZE]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // todo: make size parameter
        let texture_extent = wgpu::Extent3d {
            width: 2000,
            height: 2000,
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
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
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
                    resource: wgpu::BindingResource::TextureView(&some_view),
                },
            ],
        });

        Self {
            input_buffer,
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
        tiles: Vec<DecodedTilePlacement>,
    ) {
        for chunked_tiles in tiles.chunks(MAX_WORKGROUP_DISPATCH_SIZE) {
            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("texture_update_by_coords compute pass"),
                });
                cpass.set_pipeline(&self.compute_pipeline);
                cpass.set_bind_group(0, &self.bind_group, &[]);
                cpass.dispatch_workgroups(chunked_tiles.len() as u32, 1, 1);
            }

            let mut mapped_tiles: Vec<u32> = Vec::new();

            for tile in chunked_tiles {
                mapped_tiles.push(tile.x.into());
                mapped_tiles.push(tile.y.into());
                mapped_tiles.push(tile.color[0].into());
                mapped_tiles.push(tile.color[1].into());
                mapped_tiles.push(tile.color[2].into());
            }

            let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&mapped_tiles),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });

            encoder.copy_buffer_to_buffer(
                &staging_buffer,
                0,
                &self.input_buffer,
                0,
                (mapped_tiles.len() * std::mem::size_of::<u32>()) as u64,
            );
        }
    }
}
