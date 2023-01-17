use std::num::NonZeroU32;

use crate::{
    color_buffer_cache::ColorBufferCache,
    renderers::{ScalingRenderer, SurfaceSize},
};
use ultraviolet::{Mat4, Vec3};
use wgpu::{
    Device, ImageCopyBuffer, ImageCopyTexture, ImageDataLayout,
    Queue, Surface, Texture, TextureView,
};
use winit::window::Window;

pub struct PixelArtDisplayState {
    surface: Surface,
    adapter: wgpu::Adapter,
    device: Device,
    queue: Queue,
    texture: Texture,
    texture_view: TextureView,

    /// A default renderer to scale the input texture to the screen size (stolen from the pixels crate)
    scaling_renderer: ScalingRenderer,
    pending_texture_updates: Vec<(u32, u32, [u8; 4])>,

    current_scale_factor: f32,
    current_x_offset: f32,
    current_y_offset: f32,
    current_size: (u32, u32),

    color_buffer_cache: ColorBufferCache,
}

impl PixelArtDisplayState {
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
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST,
            label: None,
        };
        let texture = device.create_texture(&texture_desc);
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let surface_texture_format = *surface
            .get_supported_formats(&adapter)
            .first()
            .unwrap_or(&wgpu::TextureFormat::Bgra8UnormSrgb);

        let scaling_renderer = ScalingRenderer::new(
            &device,
            &texture_view,
            &texture_extent,
            &SurfaceSize {
                width: 2000,
                height: 2000,
            },
            surface_texture_format,
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
            current_scale_factor: 1.0,
            current_x_offset: 0.0,
            current_y_offset: 0.0,
            current_size: (2000, 2000),
            color_buffer_cache: ColorBufferCache::new(),
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
        let data_layout = ImageDataLayout {
            offset: 0,
            bytes_per_row: NonZeroU32::new(256),
            rows_per_image: None,
        };

        for (x, y, color) in self.pending_texture_updates.drain(..) {
            encoder.copy_buffer_to_texture(
                ImageCopyBuffer {
                    buffer: self.color_buffer_cache.get(&color, &self.device),
                    layout: data_layout,
                },
                ImageCopyTexture {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: x, y: y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
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

    pub fn resize_surface(&mut self, width: u32, height: u32) {
        self.current_size = (width, height);

        self.reconfigure_surface();
        self.update_transform_matrix();
    }

    fn reconfigure_surface(&mut self) {
        let (width, height) = self.current_size;

        self.surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.surface.get_supported_formats(&self.adapter)[0],
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
            },
        );
    }

    pub fn apply_scale_diff(&mut self, scale_diff: f32) {
        self.current_scale_factor = self.current_scale_factor + scale_diff;

        self.update_transform_matrix();
    }

    pub fn apply_translate_diff(&mut self, x_diff: f32, y_diff: f32) {
        self.current_x_offset = self.current_x_offset + x_diff;
        self.current_y_offset = self.current_y_offset + y_diff;

        self.update_transform_matrix();
    }

    fn update_transform_matrix(&mut self) {
        let (screen_width, screen_height) = self.current_size;

        let base_scale = Mat4::from_nonuniform_scale(Vec3 {
            x: 2000.0 / screen_width as f32,
            y: 2000.0 / screen_height as f32,
            z: 0.0,
        });

        let scale = Mat4::from_scale(self.current_scale_factor);
        let translate = Mat4::from_translation(ultraviolet::Vec3::new(
            self.current_x_offset,
            self.current_y_offset,
            0.0,
        ));

        let transform = translate * base_scale * scale;

        self.scaling_renderer
            .update_transform_matrix(&self.queue, transform);
    }
}
