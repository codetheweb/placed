use std::{
    fs::File,
    io::{Read, Seek},
    num::NonZeroU32,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use archive::PlacedArchiveReader;
use game_loop::{game_loop, Time, TimeTrait};
use pixels::{Pixels, SurfaceTexture};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

mod ai;
mod renderers;

struct Player<'a> {
    rendered_up_to_ms: u32,
    r: PlacedArchiveReader<'a, File>,
    render_state: ai::PixelArtRenderer,
}

impl<'a> Player<'a> {
    pub fn new(r: PlacedArchiveReader<'a, File>, render_state: ai::PixelArtRenderer) -> Self {
        Self {
            rendered_up_to_ms: 0,
            r,
            render_state,
        }
    }

    pub fn tick(&mut self) {
        self.rendered_up_to_ms += 1000 * 60;
    }

    pub fn draw(&mut self) {
        for tile in self.r.next() {
            if tile.ms_since_epoch > self.rendered_up_to_ms {
                break;
            }

            // let index = ((tile.x as u32 + (tile.y as u32) * 2000) * 4) as usize;

            self.render_state
                .update_pixel(tile.x as u32, tile.y as u32, tile.color);
        }
    }

    pub fn render(&mut self) {
        self.render_state.render();
    }
}

pub const FPS: usize = 60;
pub const TIME_STEP: Duration = Duration::from_nanos(1_000_000_000 / FPS as u64);
// Internally, the game advances at 60 fps
const ONE_FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);

pub fn play(archive_path: String) -> i32 {
    let mut event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();

    let WIDTH = 2000;
    let HEIGHT = 2000;

    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        let scaled_size = LogicalSize::new(WIDTH as f64 * 3.0, HEIGHT as f64 * 3.0);
        WindowBuilder::new()
            .with_title("Placed")
            .with_inner_size(scaled_size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    // let mut pixels = {
    //     let window_size = window.inner_size();
    //     let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    //     Pixels::new(WIDTH, HEIGHT, surface_texture).unwrap()
    // };

    // pixels.context().queue.w;

    let file = File::open(archive_path).expect("Failed to open archive");
    let mut reader = PlacedArchiveReader::new(file).expect("Failed to create reader");
    // let r = reader.into_iter();

    // let mut r = TickedPlayer::new(reader);

    // let mut is_first_render = true;

    // let game = Game {};

    let state = ai::PixelArtRenderer::new(&window);
    let p = Player::new(reader, state);

    // let state = state::State::new(&window);

    game_loop(
        event_loop,
        window,
        p,
        FPS as u32,
        0.1,
        move |g| {
            g.game.tick();
            // g.game.update_pixel(x, y, [0xffu8, 0xffu8, 0xffu8, 0xffu8]);
            // g.game.tick();
        },
        move |g| {
            g.game.draw();
            g.game.render();
            // pixels.render().unwrap();
            // g.game.draw(pixels.get_frame_mut());
            let dt = TIME_STEP.as_secs_f64() - Time::now().sub(&g.current_instant());
            if dt > 0.0 {
                std::thread::sleep(Duration::from_secs_f64(dt));
            }
        },
        |g, event| {
            match event {
                Event::WindowEvent { event, .. } => {
                    match event {
                        WindowEvent::Resized(physical_size) => {
                            // g.game.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            // new_inner_size is &&mut so we have to dereference it twice
                            // g.game.resize(**new_inner_size);
                        }
                        _ => (),
                    }
                }
                _ => (),
            };
        },
    );

    // let mut up_to_ms = 0;
    // event_loop.run(move |event, _, control_flow| {
    //     if let Event::RedrawRequested(_) = event {
    //         if is_first_render {
    //             // Fill the screen with white
    //             pixels.get_frame_mut().fill(0xff);
    //             is_first_render = false;
    //         }

    //         // let mut reader = &mut reader;
    //         // for tile in player.take_unrendered_tiles_for_tick() {
    //         // let t = r.take_unrendered_tiles_for_tick();
    //         for tile in peekable_reader.next() {
    //             let index = ((tile.x as u32 + (tile.y as u32) * WIDTH) * 4) as usize;

    //             (pixels.get_frame_mut() as &mut [u8])[index..index + 4]
    //                 .copy_from_slice(&tile.color);

    //             match peekable_reader.peek() {
    //                 Some(next_tile) => {
    //                     if next_tile.ms_since_epoch > up_to_ms {
    //                       println!("{} > {}", next_tile.ms_since_epoch, up_to_ms);
    //                         up_to_ms = next_tile.ms_since_epoch + 1000;
    //                         break;
    //                     }
    //                 }
    //                 None => break,
    //             }
    //         }

    //         // pixels.render().unwrap();
    //     }

    //     let dt = TIME_STEP.as_secs_f64() - Time::now().sub(&g.current_instant());
    //         if dt > 0.0 {
    //             std::thread::sleep(Duration::from_secs_f64(dt));
    //         }

    //     // player.tick();

    //     // r.tick();

    //     window.request_redraw();
    // })
}
