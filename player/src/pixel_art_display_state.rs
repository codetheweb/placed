use std::{
    io::{Read, Seek},
    time::Duration,
};

use crate::{
    renderers::ScalingRenderer,
    texture_update_by_coords::{PartialUpdateResult, TextureUpdateByCoords},
};
use archive::structures::Meta;
use ultraviolet::Mat4;
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

    pub texture_size: wgpu::Extent3d,
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
            width: window.inner_size().width,
            height: window.inner_size().height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
        };
        surface.configure(&device, &config);

        let canvas_size = meta
            .get_largest_canvas_size()
            .expect("No canvas size found in meta");

        let texture_extent = wgpu::Extent3d {
            width: canvas_size.width.into(),
            height: canvas_size.height.into(),
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
            texture_size: texture_extent,
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

    pub fn render(&mut self, transform: Mat4) {
        let frame = self.surface.get_current_texture().unwrap();

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        self.scaling_renderer
            .update_transform_matrix(&self.queue, transform);

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

    pub fn on_window_resize(&mut self, new_width: u32, new_height: u32) {
        self.surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: [self.surface.get_capabilities(&self.adapter).formats[0]].to_vec(),
                format: self.surface.get_capabilities(&self.adapter).formats[0],
                width: new_width,
                height: new_height,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
            },
        );
    }
}
