use std::{
    fs::File,
    io::{Read, Seek},
    time::Duration,
};

use archive::PlacedArchiveReader;
use controls::Controls;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

mod controls;
mod pixel_art_display_state;
mod renderers;
mod texture_update_by_coords;
mod transform_generator;

struct Player<R> {
    rendered_up_to: Duration,
    render_state: pixel_art_display_state::PixelArtDisplayState<R>,
    transform_generator: transform_generator::TransformGenerator,
    pub platform: egui_winit_platform::Platform,
    controls: Controls,
    egui_rpass: egui_wgpu_backend::RenderPass,
}

impl<R: Read + Seek> Player<R> {
    pub fn new(
        render_state: pixel_art_display_state::PixelArtDisplayState<R>,
        window: &winit::window::Window,
    ) -> Self {
        let texture_size = render_state.texture_size.clone();

        let platform =
            egui_winit_platform::Platform::new(egui_winit_platform::PlatformDescriptor {
                physical_width: window.inner_size().width,
                physical_height: window.inner_size().height,
                scale_factor: window.scale_factor(),
                font_definitions: egui::FontDefinitions::default(),
                style: Default::default(),
            });

        let egui_rpass = egui_wgpu_backend::RenderPass::new(
            &render_state.device,
            render_state.texture_format,
            1,
        );

        Self {
            rendered_up_to: Duration::ZERO,
            render_state,
            transform_generator: transform_generator::TransformGenerator::new(
                window.inner_size().width,
                window.inner_size().height,
                texture_size,
            ),
            platform,
            controls: controls::Controls::new(),
            egui_rpass,
        }
    }

    pub fn update(&mut self, dt: Duration) {
        self.rendered_up_to += dt * self.controls.timescale_factor as u32;

        self.render_state
            .update(self.rendered_up_to.as_millis() as u32);
    }

    pub fn draw(&mut self, window: &winit::window::Window) {
        self.transform_generator.update();

        let output_frame = match self.render_state.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                // This error occurs when the app is minimized on Windows.
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain must be updated"
                return;
            }
            Err(e) => {
                eprintln!("Dropped frame with error: {}", e);
                return;
            }
        };

        let output_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.render_state
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("encoder"),
                });

        self.render_state.scaling_renderer.update_transform_matrix(
            &self.render_state.queue,
            self.transform_generator.get_transform_matrix(),
        );

        self.render_state
            .scaling_renderer
            .render(&mut encoder, &output_view);

        self.platform.begin_frame();

        self.controls.ui(&mut self.platform.context());

        // End the UI frame. We could now handle the output and draw the UI with the backend.
        let full_output = self.platform.end_frame(Some(&window));
        let paint_jobs = self.platform.context().tessellate(full_output.shapes);

        // Upload all resources for the GPU.
        let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
            physical_width: self.render_state.surface_config.width,
            physical_height: self.render_state.surface_config.height,
            scale_factor: window.scale_factor() as f32,
        };
        let tdelta: egui::TexturesDelta = full_output.textures_delta;
        self.egui_rpass
            .add_textures(&self.render_state.device, &self.render_state.queue, &tdelta)
            .expect("add texture ok");
        self.egui_rpass.update_buffers(
            &self.render_state.device,
            &self.render_state.queue,
            &paint_jobs,
            &screen_descriptor,
        );

        // Record all render passes.
        self.egui_rpass
            .execute(
                &mut encoder,
                &output_view,
                &paint_jobs,
                &screen_descriptor,
                None,
            )
            .unwrap();
        // Submit the commands.
        self.render_state.queue.submit(Some(encoder.finish()));

        // Redraw egui
        output_frame.present();

        self.egui_rpass
            .remove_textures(tdelta)
            .expect("remove texture ok");
    }

    pub fn handle_input(&mut self, input: &WinitInputHelper) {
        let scrolled = input.scroll_diff();

        if scrolled != 0.0 {
            self.transform_generator
                .apply_scale_diff(scrolled, input.mouse());
        }

        if input.mouse_pressed(0) {
            self.transform_generator.on_pan_start();
        }

        if input.mouse_released(0) {
            self.transform_generator.on_pan_end();
        }

        if input.mouse_held(0) {
            let (x, y) = input.mouse_diff();
            self.transform_generator.apply_translate_diff(x, -y)
        }
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.render_state.on_window_resize(size.width, size.height);
        self.transform_generator
            .on_window_resize(size.width, size.height)
    }

    pub fn on_scale_factor_changed(&mut self, scale_factor: f64) {
        self.transform_generator
            .set_window_scale_factor(scale_factor as f32)
    }
}

// todo: add option to unlock fps?
pub const FPS: usize = 60;
pub const TIME_STEP: Duration = Duration::from_nanos(1_000_000_000 / FPS as u64);

// todo: make dynamic
const WIDTH: u32 = 2000;
const HEIGHT: u32 = 2000;

pub fn play(archive_path: String, timescale_factor: f32) -> i32 {
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();

    let window = {
        // todo: why is / 2 needed
        let size = PhysicalSize::new(WIDTH as f64 / 2.0, HEIGHT as f64 / 2.0);
        WindowBuilder::new()
            .with_title("Placed")
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let file = File::open(archive_path).expect("Failed to open archive");
    let reader = PlacedArchiveReader::new(file).expect("Failed to create reader");

    let mut state =
        pixel_art_display_state::PixelArtDisplayState::new(&window, reader.meta.clone(), reader);
    state.clear(wgpu::Color::WHITE);

    let mut p = Player::new(state, &window);

    event_loop.run(move |event, _, control_flow| {
        p.platform.handle_event(&event);

        match event {
            Event::RedrawRequested(..) => {
                p.update(Duration::from_secs_f64(0.01));
                p.draw(&window);
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(physical_size) => {
                    p.resize(physical_size);
                }
                WindowEvent::ScaleFactorChanged {
                    scale_factor,
                    new_inner_size,
                } => {
                    p.on_scale_factor_changed(scale_factor);
                    println!("scale_factor: {:?}", new_inner_size);
                    p.resize(*new_inner_size);
                }
                _ => {}
            },
            _ => {}
        }
    })
}
