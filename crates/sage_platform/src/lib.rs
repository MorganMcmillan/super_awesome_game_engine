//! Presentation: window, keyboard and drawing on SDL3.
//!
//! Reads simulation state only through `sage_core::host::Frame` and feeds the
//! simulation only `TickInput`, which is recorded per tick. Wall-clock time
//! lives here and decides *how many* ticks to run per frame, never what a
//! tick computes, so a recorded run replays identically headless.
//!
//! Milestone 0 draws with SDL's simple Render API (spike S5 recommendation,
//! docs/spikes/S5.md); the GPU renderer replaces `SdlRenderer` in M1 behind
//! the same `Renderer` trait.

use std::time::{Duration, Instant};

use sage_core::host::{DrawKind, Frame};
use sage_core::{InputRecording, Simulation, TICK_DT, TickInput};
use sdl3::event::Event;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::pixels::Color;
use sdl3::render::FRect;
use sdl3::render::WindowCanvas;

pub use sdl3::keyboard::Scancode as Key;

/// Draws a frame. Implementations must not affect the simulation.
pub trait Renderer {
    fn draw(&mut self, frame: &Frame) -> Result<(), String>;
}

/// A simulation that can describe itself to presentation.
pub trait Presentable: Simulation {
    fn frame(&self) -> Frame;
}

/// Maps held keys to input bits. The game owns the meaning of the bits.
#[derive(Clone, Debug, Default)]
pub struct InputMap {
    pub bindings: Vec<(Scancode, u32)>,
}

impl InputMap {
    pub fn bind(mut self, key: Scancode, bit: u32) -> Self {
        self.bindings.push((key, bit));
        self
    }

    fn sample(&self, held: impl Fn(Scancode) -> bool) -> TickInput {
        self.bindings
            .iter()
            .fold(TickInput::EMPTY, |acc, &(key, bit)| {
                acc.with(bit, held(key))
            })
    }
}

pub fn color_of(kind: DrawKind) -> Color {
    match kind {
        DrawKind::Court => Color::RGB(16, 20, 28),
        DrawKind::Wall => Color::RGB(120, 130, 150),
        DrawKind::Goal => Color::RGB(60, 30, 40),
        DrawKind::Paddle => Color::RGB(235, 235, 240),
        DrawKind::Ball => Color::RGB(255, 200, 60),
    }
}

pub const BACKGROUND: Color = Color::RGB(0, 0, 0);

/// Draws rectangles and HUD text with SDL's Render API.
pub struct SdlRenderer {
    canvas: WindowCanvas,
}

impl SdlRenderer {
    pub fn new(canvas: WindowCanvas) -> Self {
        Self { canvas }
    }

    /// Counts pixels per `DrawKind` color in the last drawn frame (before
    /// present). Slow; for smoke tests that check the court was drawn.
    pub fn count_colored_pixels(&self) -> Result<Vec<(DrawKind, usize)>, String> {
        let surface = self
            .canvas
            .read_pixels(None)
            .map_err(|e| e.to_string())?
            .convert_format(sdl3::pixels::PixelFormat::RGBA32)
            .map_err(|e| e.to_string())?;
        let (w, pitch) = (surface.width() as usize, surface.pitch() as usize);
        let kinds = [
            DrawKind::Court,
            DrawKind::Wall,
            DrawKind::Goal,
            DrawKind::Paddle,
            DrawKind::Ball,
        ];
        Ok(surface.with_lock(|bytes| {
            kinds
                .iter()
                .map(|&k| {
                    let c = color_of(k);
                    let n = bytes
                        .chunks(pitch)
                        .flat_map(|row| row[..w * 4].chunks(4))
                        .filter(|p| p[0] == c.r && p[1] == c.g && p[2] == c.b)
                        .count();
                    (k, n)
                })
                .collect()
        }))
    }

    pub fn present(&mut self) {
        self.canvas.present();
    }
}

impl Renderer for SdlRenderer {
    fn draw(&mut self, frame: &Frame) -> Result<(), String> {
        let (out_w, out_h) = self.canvas.output_size().map_err(|e| e.to_string())?;
        let [ww, wh] = frame.world_size;
        let hud_h = 24.0;
        // Fit the world (plus a margin for the goal zones) into the window,
        // y up in the world, y down on screen.
        let margin = 4.0;
        let scale =
            (out_w as f32 / (ww + 2.0 * margin)).min((out_h as f32 - hud_h) / (wh + 2.0 * margin));
        let ox = (out_w as f32 - ww * scale) / 2.0;
        let oy = hud_h + (out_h as f32 - hud_h - wh * scale) / 2.0;
        self.canvas.set_draw_color(BACKGROUND);
        self.canvas.clear();
        // Court first so everything else draws over it.
        let mut rects: Vec<_> = frame.rects.iter().collect();
        rects.sort_by_key(|r| (r.kind != DrawKind::Court, r.id));
        for r in rects {
            self.canvas.set_draw_color(color_of(r.kind));
            let x = ox + (r.center[0] - r.half[0]) * scale;
            let y = oy + (wh - (r.center[1] + r.half[1])) * scale;
            self.canvas
                .fill_rect(FRect::new(
                    x,
                    y,
                    2.0 * r.half[0] * scale,
                    2.0 * r.half[1] * scale,
                ))
                .map_err(|e| e.to_string())?;
        }
        self.canvas.set_draw_color(Color::RGB(200, 200, 200));
        let text = frame.hud.join("    ");
        let tx = (out_w as f32 - text.len() as f32 * 8.0) / 2.0;
        self.canvas
            .draw_debug_text(&text, (tx.max(4.0), 8.0))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct WindowOptions {
    pub title: String,
    pub size: (u32, u32),
    /// Stop after this many rendered frames (smoke tests). `None` runs until
    /// the window closes or Escape is pressed.
    pub max_frames: Option<u64>,
    pub hidden: bool,
}

#[derive(Debug)]
pub struct WindowRun {
    pub recording: InputRecording,
    pub frames: u64,
    /// Pixel counts from the last frame, when `max_frames` was set.
    pub last_frame_pixels: Option<Vec<(DrawKind, usize)>>,
}

/// Runs `sim` in a window at the fixed tick rate, recording input per tick.
/// `seed` is written into the recording so it can be replayed headless.
pub fn run_windowed<S: Presentable>(
    sim: &mut S,
    seed: u64,
    input_map: &InputMap,
    options: &WindowOptions,
) -> Result<WindowRun, String> {
    let sdl = sdl3::init().map_err(|e| e.to_string())?;
    let video = sdl.video().map_err(|e| e.to_string())?;
    let mut builder = video.window(&options.title, options.size.0, options.size.1);
    builder.position_centered().resizable();
    if options.hidden {
        builder.hidden();
    }
    let window = builder.build().map_err(|e| e.to_string())?;
    let mut renderer = SdlRenderer::new(window.into_canvas());
    let mut events = sdl.event_pump().map_err(|e| e.to_string())?;

    let mut recording = InputRecording::new(seed);
    let tick_len = Duration::from_secs_f32(TICK_DT);
    let mut last = Instant::now();
    let mut accumulator = Duration::ZERO;
    let mut frames = 0u64;
    let mut last_frame_pixels = None;

    'run: loop {
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'run,
                _ => {}
            }
        }
        let now = Instant::now();
        // Smoke runs advance exactly one tick per frame, independent of speed.
        if options.max_frames.is_some() {
            accumulator = tick_len;
        } else {
            accumulator += (now - last).min(Duration::from_millis(250));
        }
        last = now;
        while accumulator >= tick_len {
            let keys = events.keyboard_state();
            let input = input_map.sample(|k| keys.is_scancode_pressed(k));
            recording.record(sim.tick(), input);
            sim.step(input);
            accumulator -= tick_len;
        }
        renderer.draw(&sim.frame())?;
        frames += 1;
        if let Some(max) = options.max_frames
            && frames >= max
        {
            last_frame_pixels = Some(renderer.count_colored_pixels()?);
            renderer.present();
            break;
        }
        renderer.present();
        std::thread::sleep(Duration::from_millis(1));
    }
    // Make the recording cover every tick that ran, even trailing idle ones.
    if sim.tick() > 0 {
        let last_tick = sim.tick() - 1;
        recording.record(last_tick, recording.get(last_tick));
    }
    Ok(WindowRun {
        recording,
        frames,
        last_frame_pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_map_sets_bits() {
        let map = InputMap::default()
            .bind(Scancode::W, 1)
            .bind(Scancode::S, 2);
        assert_eq!(map.sample(|k| k == Scancode::S).buttons, 2);
        assert_eq!(map.sample(|_| true).buttons, 3);
        assert_eq!(map.sample(|_| false), TickInput::EMPTY);
    }
}
