use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    time::Duration,
};

use archive::{structures::StoredTilePlacement, PlacedArchiveReader};
use game_loop::{game_loop, Time, TimeTrait};
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

mod pixel_art_display_state;
mod renderers;
mod texture_update_by_coords;
mod transform_generator;

struct Player<R> {
    rendered_up_to: Duration,
    render_state: pixel_art_display_state::PixelArtDisplayState<R>,
    transform_generator: transform_generator::TransformGenerator,
    timescale_factor: f32,
}

impl<R: Read + Seek> Player<R> {
    pub fn new(
        render_state: pixel_art_display_state::PixelArtDisplayState<R>,
        timescale_factor: f32,
        window_size: PhysicalSize<u32>,
    ) -> Self {
        let texture_size = render_state.texture_size.clone();
        Self {
            rendered_up_to: Duration::ZERO,
            render_state,
            timescale_factor,
            transform_generator: transform_generator::TransformGenerator::new(
                window_size.width,
                window_size.height,
                texture_size,
            ),
        }
    }

    pub fn update(&mut self, dt: Duration) {
        self.rendered_up_to += dt * self.timescale_factor as u32;

        self.render_state
            .update(self.rendered_up_to.as_millis() as u32);
    }

    pub fn draw(&mut self) {
        self.transform_generator.update();
        self.render_state
            .render(self.transform_generator.get_transform_matrix());
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
        let size = LogicalSize::new(WIDTH as f64 / 2.0, HEIGHT as f64 / 2.0);
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
    let p = Player::new(state, timescale_factor, window.inner_size());

    game_loop(
        event_loop,
        window,
        p,
        FPS as u32,
        2.0,
        move |g| {
            g.game.update(TIME_STEP);
        },
        move |g| {
            g.game.draw();

            let dt = TIME_STEP.as_secs_f64() - Time::now().sub(&g.current_instant());
            if dt > 0.0 {
                std::thread::sleep(Duration::from_secs_f64(dt));
            }
        },
        move |g, event| {
            if input.update(event) {
                g.game.handle_input(&input);
            }

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Resized(physical_size) => {
                        g.game.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged {
                        new_inner_size,
                        scale_factor,
                        ..
                    } => {
                        g.game.on_scale_factor_changed(*scale_factor);
                        g.game.resize(**new_inner_size);
                    }
                    WindowEvent::CloseRequested => {
                        std::process::exit(0);
                    }
                    _ => (),
                },
                _ => {}
            };
        },
    );
}
