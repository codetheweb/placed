use std::{
    cmp::min,
    mem::size_of,
    num::{NonZeroU32, NonZeroU64},
    vec,
};

use archive::structures::{Meta, StoredTilePlacement};
use wgpu::util::DeviceExt;

pub struct TextureUpdateByCoords {
    texture: wgpu::Texture,
    texture_extent: wgpu::Extent3d,
    pub texture_view: wgpu::TextureView,
    input_buffer: wgpu::Buffer,
    zeros_buffer: wgpu::Buffer,
    calculate_final_tiles_pipeline: wgpu::ComputePipeline,
    calculate_final_tiles_bind_group: wgpu::BindGroup,
    update_texture_pipeline: wgpu::ComputePipeline,
    update_texture_bind_group: wgpu::BindGroup,
    last_index_for_tile: wgpu::Buffer,
    staging_belt: wgpu::util::StagingBelt,
}

const NUM_OF_TILES_PER_WORKGROUP: u32 = 4;

// todo: add note about assuming sorted input

impl TextureUpdateByCoords {
    pub fn new(device: &wgpu::Device, meta: Meta) -> Self {
        let shader = wgpu::include_wgsl!("../shaders/texture_update_by_coords.compute.wgsl");
        let module = device.create_shader_module(shader);

        let calculate_final_tiles_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("texture_update_by_coords calculate_final_tiles_pipeline"),
                layout: None,
                module: &module,
                entry_point: "calculate_final_tiles",
            });

        let update_texture_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("texture_update_by_coords update_texture_pipeline"),
                layout: None,
                module: &module,
                entry_point: "update_texture",
            });

        let max_input_size = TextureUpdateByCoords::get_max_num_of_tiles_per_chunk(&device)
            * (StoredTilePlacement::encoded_size() as u32);

        let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("texture_update_by_coords input buffer"),
            contents: bytemuck::cast_slice(&vec![0u32; max_input_size.try_into().unwrap()]),
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

        let size = meta.get_largest_canvas_size().unwrap();

        let mut r = r.into_iter().flatten().collect::<Vec<u32>>();
        // Padding for alignment
        r.append(&mut vec![size.width.into(), size.height.into()]);
        r.append(&mut vec![0; 2]);

        let locals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("texture_update_by_coords locals buffer"),
            contents: bytemuck::cast_slice(&r),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

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
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
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

        let z = vec![0u32; size.width as usize * size.height as usize];

        let last_index_for_tile = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("texture_update_by_coords last index buffer"),
            contents: bytemuck::cast_slice(&z),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let zeros_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("texture_update_by_coords zeros buffer buffer"),
            contents: bytemuck::cast_slice(&z),
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        let calculate_final_tiles_bind_group_layout =
            calculate_final_tiles_pipeline.get_bind_group_layout(0);
        let calculate_final_tiles_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &calculate_final_tiles_bind_group_layout,
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
                        resource: last_index_for_tile.as_entire_binding(),
                    },
                ],
            });

        let update_texture_bind_group_layout = update_texture_pipeline.get_bind_group_layout(0);
        let update_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &update_texture_bind_group_layout,
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
                    resource: last_index_for_tile.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&some_view),
                },
            ],
        });

        Self {
            input_buffer,
            texture,
            texture_extent,
            texture_view: some_view,
            calculate_final_tiles_pipeline,
            calculate_final_tiles_bind_group,
            update_texture_pipeline,
            update_texture_bind_group,
            zeros_buffer,
            last_index_for_tile,
            // todo: use correct chunk size
            staging_belt: wgpu::util::StagingBelt::new(max_input_size.into()),
        }
    }

    /// Make sure to only pass one tile per position, as it's not guaranteed that the order of tiles will be preserved during rendering.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        chunk: Vec<u8>,
    ) {
        self.staging_belt.recall();

        let num_of_tiles = chunk.len() / StoredTilePlacement::encoded_size();

        let mut current_tile_offset = 0;
        while current_tile_offset < num_of_tiles {
            let next_tile_offset = min(
                current_tile_offset
                    + (TextureUpdateByCoords::get_max_num_of_tiles_per_chunk(device) as usize),
                num_of_tiles,
            );
            let current = &chunk[(current_tile_offset * StoredTilePlacement::encoded_size())
                ..(next_tile_offset * StoredTilePlacement::encoded_size())];

            // Pad
            let mut current = current.to_vec();
            while current.len()
                % ((NUM_OF_TILES_PER_WORKGROUP as usize) * StoredTilePlacement::encoded_size())
                != 0
            {
                StoredTilePlacement {
                    x: 0,
                    y: 0,
                    color_index: 255,
                    ms_since_epoch: 0,
                }
                .write_into(&mut current);
            }

            {
                let mut s = self.staging_belt.write_buffer(
                    encoder,
                    &self.input_buffer,
                    0,
                    NonZeroU64::new(current.len() as u64).unwrap(),
                    device,
                );
                s.copy_from_slice(&current);
            }

            self.staging_belt.finish();

            let num_of_workgroups = f32::ceil(
                (next_tile_offset - current_tile_offset) as f32 / NUM_OF_TILES_PER_WORKGROUP as f32,
            ) as u32;

            // Clear state data from last chunk
            encoder.copy_buffer_to_buffer(
                &self.zeros_buffer,
                0,
                &self.last_index_for_tile,
                0,
                self.zeros_buffer.size(),
            );

            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("texture_update_by_coords.calculate_final_tiles compute pass"),
                });
                cpass.set_pipeline(&self.calculate_final_tiles_pipeline);
                cpass.set_bind_group(0, &self.calculate_final_tiles_bind_group, &[]);

                cpass.dispatch_workgroups(num_of_workgroups, NUM_OF_TILES_PER_WORKGROUP, 1);
            }

            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("texture_update_by_coords.update_texture compute pass"),
                });
                cpass.set_pipeline(&self.update_texture_pipeline);
                cpass.set_bind_group(0, &self.update_texture_bind_group, &[]);

                cpass.dispatch_workgroups(num_of_workgroups, NUM_OF_TILES_PER_WORKGROUP, 1);
            }

            current_tile_offset = next_tile_offset;
        }
    }

    fn get_max_num_of_tiles_per_chunk(device: &wgpu::Device) -> u32 {
        device.limits().max_compute_workgroups_per_dimension * NUM_OF_TILES_PER_WORKGROUP
    }
}

#[cfg(test)]
mod tests {
    use archive::structures::{CanvasSizeChange, Meta, StoredTilePlacement};
    use image::{ImageBuffer, Rgba};
    use log::{log_enabled, Level};
    use rand::Rng;
    use std::{collections::BTreeMap, num::NonZeroU32, sync::mpsc};
    use wgpu::{CommandEncoder, Device};

    use super::TextureUpdateByCoords;

    struct TestHelpers {}

    impl TestHelpers {
        pub fn render_to_buffer<F>(
            test_name: &str,
            meta: Meta,
            callback: F,
        ) -> ImageBuffer<Rgba<u8>, Vec<u8>>
        where
            F: FnOnce(&Device, &mut CommandEncoder, &mut TextureUpdateByCoords),
        {
            let (device, queue) = Self::get_device();

            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

            let mut controller = TextureUpdateByCoords::new(&device, meta.clone());
            callback(&device, &mut encoder, &mut controller);

            let u32_size = std::mem::size_of::<u32>() as u32;

            let texture_size = meta.get_largest_canvas_size().unwrap();

            let output_buffer_size =
                (u32_size * texture_size.width as u32 * texture_size.height as u32)
                    as wgpu::BufferAddress;
            let output_buffer_desc = wgpu::BufferDescriptor {
                size: output_buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                label: None,
                mapped_at_creation: false,
            };
            let output_buffer = device.create_buffer(&output_buffer_desc);

            encoder.copy_texture_to_buffer(
                wgpu::ImageCopyTexture {
                    aspect: wgpu::TextureAspect::All,
                    texture: &controller.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                },
                wgpu::ImageCopyBuffer {
                    buffer: &output_buffer,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: NonZeroU32::new(u32_size * texture_size.width as u32),
                        rows_per_image: NonZeroU32::new(texture_size.height as u32),
                    },
                },
                controller.texture_extent,
            );

            queue.submit(Some(encoder.finish()));

            let buffer_slice = output_buffer.slice(..);

            let (tx, rx) = mpsc::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).unwrap();
            });
            device.poll(wgpu::Maintain::Wait);
            rx.recv().unwrap().unwrap();

            let data = buffer_slice.get_mapped_range();

            let buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(
                texture_size.width as u32,
                texture_size.height as u32,
                // copy data to avoid dealing with lifetimes
                data.to_vec(),
            )
            .unwrap();

            env_logger::try_init().ok();

            if log_enabled!(Level::Debug) {
                buffer.save(format!("{}.png", test_name)).unwrap();
            }

            buffer
        }

        pub fn get_device() -> (Device, wgpu::Queue) {
            pollster::block_on(Self::get_device_async())
        }
        async fn get_device_async() -> (Device, wgpu::Queue) {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());

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

    #[test]
    fn black_rows() {
        let mut color_id_to_tuple = BTreeMap::new();
        color_id_to_tuple.insert(0, [0, 0, 0, 255]);

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

        // Every other row with black
        for x in 0..texture_size {
            for y in 0..texture_size {
                if y % 2 == 0 {
                    continue;
                }

                let tile = StoredTilePlacement {
                    x: x as u16,
                    y: y as u16,
                    color_index: 0,
                    ms_since_epoch: 0,
                };

                tile.write_into(&mut data);
            }
        }

        let buffer =
            TestHelpers::render_to_buffer("black_rows", meta, |device, encoder, controller| {
                controller.update(device, encoder, data);
            });

        // Check generated texture
        for x in 0..texture_size {
            for y in 0..texture_size {
                if y % 2 == 0 {
                    assert_eq!(buffer.get_pixel(x, y), &Rgba([0, 0, 0, 0]));
                } else {
                    assert_eq!(buffer.get_pixel(x, y), &Rgba([0, 0, 0, 255]));
                }
            }
        }
    }

    #[test]
    fn red_square() {
        let mut color_id_to_tuple = BTreeMap::new();
        color_id_to_tuple.insert(0, [255, 0, 0, 255]);

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

        // Fill with red
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

        let buffer =
            TestHelpers::render_to_buffer("red_square", meta, |device, encoder, controller| {
                controller.update(device, encoder, data);
            });

        // Check generated texture
        for x in 0..texture_size {
            for y in 0..texture_size {
                assert_eq!(buffer.get_pixel(x, y), &Rgba([255, 0, 0, 255]));
            }
        }
    }

    #[test]
    fn single_pixel() {
        let mut color_id_to_tuple = BTreeMap::new();
        color_id_to_tuple.insert(0, [255, 0, 0, 255]);

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

        StoredTilePlacement {
            x: 63,
            y: 63,
            color_index: 0,
            ms_since_epoch: 0,
        }
        .write_into(&mut data);

        let buffer =
            TestHelpers::render_to_buffer("single_pixel", meta, |device, encoder, controller| {
                controller.update(device, encoder, data);
            });

        // Check generated texture
        for x in 0..texture_size {
            for y in 0..texture_size {
                if x == 63 && y == 63 {
                    assert_eq!(buffer.get_pixel(x, y), &Rgba([255, 0, 0, 255]));
                } else {
                    assert_eq!(buffer.get_pixel(x, y), &Rgba([0, 0, 0, 0]));
                }
            }
        }
    }

    #[test]
    fn multi_color() {
        let mut color_id_to_tuple = BTreeMap::new();
        color_id_to_tuple.insert(0, [255, 0, 0, 255]);
        color_id_to_tuple.insert(1, [0, 255, 0, 255]);
        color_id_to_tuple.insert(2, [0, 0, 255, 255]);

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

        for x in 0..texture_size {
            for y in 0..texture_size {
                let tile = StoredTilePlacement {
                    x: x as u16,
                    y: y as u16,
                    color_index: (x % 3) as u8,
                    ms_since_epoch: 0,
                };

                tile.write_into(&mut data);
            }
        }

        let buffer =
            TestHelpers::render_to_buffer("multi_color", meta, |device, encoder, controller| {
                controller.update(device, encoder, data);
            });

        // Check generated texture
        for x in 0..texture_size {
            for y in 0..texture_size {
                let expected_color = match x % 3 {
                    0 => [255, 0, 0, 255],
                    1 => [0, 255, 0, 255],
                    2 => [0, 0, 255, 255],
                    _ => unreachable!(),
                };

                assert_eq!(buffer.get_pixel(x, y), &Rgba(expected_color));
            }
        }
    }

    #[test]
    fn odd_number_of_tiles() {
        let mut color_id_to_tuple = BTreeMap::new();
        color_id_to_tuple.insert(0, [0, 0, 0, 255]);

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

        for i in 0..7 {
            StoredTilePlacement {
                x: i as u16,
                y: i as u16,
                color_index: 0,
                ms_since_epoch: 0,
            }
            .write_into(&mut data);
        }

        let buffer = TestHelpers::render_to_buffer(
            "odd_number_of_tiles",
            meta,
            |device, encoder, controller| {
                controller.update(device, encoder, data);
            },
        );

        // Check generated texture
        for x in 0..texture_size {
            for y in 0..texture_size {
                if x < 7 && y < 7 && x == y {
                    assert_eq!(buffer.get_pixel(x, y), &Rgba([0, 0, 0, 255]));
                } else {
                    assert_eq!(buffer.get_pixel(x, y), &Rgba([0, 0, 0, 0]));
                }
            }
        }
    }

    #[test]
    fn preserves_order_of_tiles_in_chunk() {
        let mut color_id_to_tuple = BTreeMap::new();
        color_id_to_tuple.insert(0, [0, 0, 0, 255]);
        color_id_to_tuple.insert(1, [255, 0, 0, 255]);

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

        for x in 0..texture_size {
            for y in 0..texture_size {
                StoredTilePlacement {
                    x: x as u16,
                    y: y as u16,
                    color_index: 0,
                    ms_since_epoch: 0,
                }
                .write_into(&mut data);

                StoredTilePlacement {
                    x: x as u16,
                    y: y as u16,
                    color_index: 1,
                    ms_since_epoch: 0,
                }
                .write_into(&mut data);
            }
        }

        let buffer = TestHelpers::render_to_buffer(
            "preserves_order_of_tiles_in_chunk",
            meta,
            |device, encoder, controller| {
                controller.update(device, encoder, data);
            },
        );

        // Check generated texture
        for x in 0..texture_size {
            for y in 0..texture_size {
                assert_eq!(buffer.get_pixel(x, y), &Rgba([255, 0, 0, 255]));
            }
        }
    }

    #[test]
    fn multiple_chunks() {
        let mut color_id_to_tuple = BTreeMap::new();
        color_id_to_tuple.insert(0, [0, 0, 0, 255]);
        color_id_to_tuple.insert(1, [255, 0, 0, 255]);

        let texture_size: u32 = 64;

        let meta = Meta {
            chunk_descs: vec![],
            color_id_to_tuple: color_id_to_tuple.clone(),
            last_pixel_placed_at_seconds_since_epoch: 0,
            canvas_size_changes: vec![CanvasSizeChange {
                width: texture_size as u16,
                height: texture_size as u16,
                ms_since_epoch: 0,
            }],
        };

        let mut expected_color_index = 0;

        let buffer = TestHelpers::render_to_buffer(
            "multiple_chunks",
            meta,
            |device, encoder, controller| {
                let required_num_of_tile_updates =
                    TextureUpdateByCoords::get_max_num_of_tiles_per_chunk(device) * 2;
                let required_num_of_full_texture_refreshes =
                    required_num_of_tile_updates / (texture_size * texture_size);

                let mut data: Vec<u8> = Vec::new();

                for i in 0..required_num_of_full_texture_refreshes {
                    if i % 2 == 0 {
                        for x in 0..texture_size {
                            for y in 0..texture_size {
                                StoredTilePlacement {
                                    x: x as u16,
                                    y: y as u16,
                                    color_index: 0,
                                    ms_since_epoch: 0,
                                }
                                .write_into(&mut data);
                            }
                        }

                        expected_color_index = 0;
                    } else {
                        for x in (0..texture_size).rev() {
                            for y in (0..texture_size).rev() {
                                StoredTilePlacement {
                                    x: x as u16,
                                    y: y as u16,
                                    color_index: 1,
                                    ms_since_epoch: 0,
                                }
                                .write_into(&mut data);
                            }
                        }

                        expected_color_index = 1;
                    }
                }

                controller.update(device, encoder, data);
            },
        );

        // Check generated texture
        let expected_color = Rgba(*color_id_to_tuple.get(&expected_color_index).unwrap());
        for x in 0..texture_size {
            for y in 0..texture_size {
                assert_eq!(buffer.get_pixel(x, y), &expected_color);
            }
        }
    }

    #[test]
    fn fuzz() {
        let mut color_id_to_tuple = BTreeMap::new();
        color_id_to_tuple.insert(0, [0, 0, 0, 255]);
        color_id_to_tuple.insert(1, [255, 0, 0, 255]);
        color_id_to_tuple.insert(2, [0, 255, 0, 255]);
        color_id_to_tuple.insert(3, [0, 0, 255, 255]);
        color_id_to_tuple.insert(4, [255, 255, 0, 255]);
        color_id_to_tuple.insert(5, [255, 0, 255, 255]);
        color_id_to_tuple.insert(6, [0, 255, 255, 255]);
        color_id_to_tuple.insert(7, [255, 255, 255, 255]);

        let texture_size: u32 = 64;

        let meta = Meta {
            chunk_descs: vec![],
            color_id_to_tuple: color_id_to_tuple.clone(),
            last_pixel_placed_at_seconds_since_epoch: 0,
            canvas_size_changes: vec![CanvasSizeChange {
                width: texture_size as u16,
                height: texture_size as u16,
                ms_since_epoch: 0,
            }],
        };

        let mut expected_texture: Vec<Vec<u8>> =
            vec![vec![0; texture_size as usize]; texture_size as usize];

        let mut generator = rand::thread_rng();

        let buffer = TestHelpers::render_to_buffer(
            "fuzz",
            meta,
            |device, encoder, controller| {
                let required_num_of_tile_updates =
                    TextureUpdateByCoords::get_max_num_of_tiles_per_chunk(device) * 2;

                let mut data: Vec<u8> = Vec::new();

                for _ in 0..required_num_of_tile_updates {
                    let x = generator.gen_range(0..texture_size);
                    let y = generator.gen_range(0..texture_size);

                    StoredTilePlacement {
                        x: x as u16,
                        y: y as u16,
                        color_index: 0,
                        ms_since_epoch: 0,
                    }
                    .write_into(&mut data);

                    let color_index = generator.gen_range(0..4);
                    StoredTilePlacement {
                        x: x as u16,
                        y: y as u16,
                        color_index,
                        ms_since_epoch: 0,
                    }
                    .write_into(&mut data);

                    expected_texture[x as usize][y as usize] = color_index;
                }

                controller.update(device, encoder, data);
            },
        );

        // Check generated texture
        for x in 0..texture_size {
            for y in 0..texture_size {
                assert_eq!(
                    buffer.get_pixel(x, y),
                    &Rgba(
                        *color_id_to_tuple
                            .get(&expected_texture[x as usize][y as usize])
                            .unwrap()
                    )
                );
            }
        }
    }
}
