use std::num::NonZeroU32;

use crate::renderers::{ScalingRenderer, SurfaceSize};
use wgpu::{
    Adapter, Device, ImageCopyTexture, ImageDataLayout, Queue, Surface, Texture, TextureView,
};
use winit::window::Window;

pub struct PixelArtRenderer {
    surface: Surface,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    texture: Texture,
    texture_view: TextureView,

    /// A default renderer to scale the input texture to the screen size (stolen from the pixels crate)
    pub scaling_renderer: ScalingRenderer,
    pending_texture_updates: Vec<(u32, u32, [u8; 4])>,
}

impl PixelArtRenderer {
    pub fn new(window: &Window) -> Self {
        pollster::block_on(Self::async_new(window))
    }

    async fn async_new(window: &Window) -> Self {
        let instance = wgpu::Instance::new(wgpu::Backends::all());

        let surface = unsafe { instance.create_surface(&window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_supported_formats(&adapter)[0],
            width: 2000,
            height: 2000,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
        };
        surface.configure(&device, &config);

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
            // format: wgpu::TextureFormat::Rgba8UnormSrgb,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST,
            label: None,
        };
        let texture = device.create_texture(&texture_desc);

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let scaling_renderer = ScalingRenderer::new(
            &device,
            &texture_view,
            &texture_extent,
            &SurfaceSize {
                width: 2000,
                height: 2000,
            },
            texture_desc.format,
            wgpu::Color::BLACK,
            wgpu::BlendState::REPLACE,
        );

        Self {
            surface,
            adapter,
            device,
            queue,
            texture,
            texture_view,
            scaling_renderer,
            pending_texture_updates: Vec::new(),
        }
    }

    pub fn update_pixel(&mut self, x: u32, y: u32, color: [u8; 4]) {
        self.pending_texture_updates.push((x, y, color));
    }

    pub fn render(&mut self) {
        let frame = self.surface.get_current_texture().unwrap();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        // Update texture
        for (x, y, color) in self.pending_texture_updates.drain(..) {
            let data_layout = ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(256),
                rows_per_image: None,
            };

            self.queue.write_texture(
                ImageCopyTexture {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: x, y: y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &color,
                data_layout,
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.scaling_renderer.render(&mut encoder, &view);

        {
            // let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            //     label: None,
            //     color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
            //         attachment: &frame.view,
            //         resolve_target: None,
            //         ops: wgpu::Operations {
            //             load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
            //             store: true,
            //         },
            //     }],
            //     depth_stencil_attachment: None,
            // });

            // render_pass.set_pipeline(&self.pipeline);
            // render_pass.set_bind_group(0, &self.bind_group, &[]);
            // render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));

        frame.present();
    }

    pub fn clear(&mut self, color: wgpu::Color) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clear_encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
