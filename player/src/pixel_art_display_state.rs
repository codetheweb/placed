use std::{
    io::{Read, Seek},
    time::Duration,
};

use crate::{
    renderers::{ScalingRenderer, SurfaceSize},
    texture_update_by_coords::{PartialUpdateResult, TextureUpdateByCoords},
};
use archive::structures::Meta;
use ultraviolet::{Mat4, Vec3};
use wgpu::{Adapter, Device, Instance, Queue, Surface};
use winit::window::Window;

pub struct PixelArtDisplayState<R> {
    surface: Surface,
    adapter: Adapter,
    device: Device,
    queue: Queue,

    /// A default renderer to scale the input texture to the screen size (stolen from the pixels crate)
    scaling_renderer: ScalingRenderer,
    compute_renderer: TextureUpdateByCoords<R>,
    last_up_to_ms: u32,
    up_to_ms: u32,

    current_scale_factor: f32,
    current_x_offset: f32,
    current_y_offset: f32,
    current_size: (u32, u32),
}

impl<R: Read + Seek> PixelArtDisplayState<R> {
    pub fn new(window: &Window, meta: Meta, reader: R) -> Self {
        pollster::block_on(Self::async_new(window, meta, reader))
    }

    async fn async_new(window: &Window, meta: Meta, reader: R) -> Self {
        let instance = Instance::new(wgpu::InstanceDescriptor::default());

        let surface = unsafe { instance.create_surface(&window).unwrap() };
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
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: [surface.get_capabilities(&adapter).formats[0]].to_vec(),
            format: surface.get_capabilities(&adapter).formats[0],
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

        let surface_texture_format = *surface
            .get_capabilities(&adapter)
            .formats
            .first()
            .unwrap_or(&wgpu::TextureFormat::Bgra8UnormSrgb);

        let compute_renderer = TextureUpdateByCoords::new(&device, meta, reader, None);

        let scaling_renderer = ScalingRenderer::new(
            &device,
            &compute_renderer.texture_view,
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
            scaling_renderer,
            compute_renderer,
            last_up_to_ms: 0,
            up_to_ms: 0,
            current_scale_factor: 0.5,
            current_x_offset: 0.0,
            current_y_offset: 0.0,
            current_size: (2000, 2000),
        }
    }

    pub fn update(&mut self, up_to_ms: u32) {
        self.last_up_to_ms = self.up_to_ms;
        self.up_to_ms = up_to_ms;

        let diff = Duration::from_millis((self.up_to_ms - self.last_up_to_ms).into());

        match self
            .compute_renderer
            .update(&self.device, &self.queue, self.up_to_ms, diff)
        {
            PartialUpdateResult::ReachedEndOfInput => {
                // temp
                panic!("Reached end of input");
            }
            PartialUpdateResult::UpdatedUpToMs {
                max_ms_since_epoch_used,
                did_update_up_to_requested_ms,
            } => {}
        }
    }

    pub fn render(&mut self) {
        let frame = self.surface.get_current_texture().unwrap();

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

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
                label: Some("Clear render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.compute_renderer.texture_view,
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
                view_formats: [self.surface.get_capabilities(&self.adapter).formats[0]].to_vec(),
                format: self.surface.get_capabilities(&self.adapter).formats[0],
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
