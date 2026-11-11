//! A window for a UI application: mouse, wheel, typed text and keyboard
//! navigation in, a [`UiFrame`] out.
//!
//! The same split as `run_windowed`: wall-clock time decides how many fixed
//! ticks run per frame and never what a tick computes. Unlike a game, the
//! input here is *not* recorded: `sage-input v1` holds button bits only, so a
//! pointer cannot be replayed yet (docs/milestone-2.md). Keyboard navigation
//! uses the `NAV_*` bits the UI already understands.

use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use sage_assets::{BitmapFont, Image};
use sage_core::TICK_DT;
use sage_core::host::{
    NAV_ACCEPT, NAV_BACKSPACE, NAV_DELETE, NAV_DOWN, NAV_END, NAV_HOME, NAV_LEFT, NAV_NEXT,
    NAV_PAGE_DOWN, NAV_PAGE_UP, NAV_PREV, NAV_RIGHT, NAV_UP, UiFrame,
};
use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::{KeyboardState, Keycode, Scancode};
use sdl3::mouse::MouseButton;

use crate::WindowOptions;
use crate::gpu::GpuRenderer;
use crate::sprite::{compose_ui, ui_view};

/// One tick of UI input, as the window loop samples it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiTickInput {
    /// The pointer in UI units, or `None` when it is outside the window.
    pub pointer: Option<[f32; 2]>,
    /// The left mouse button is held.
    pub down: bool,
    /// `NAV_*` bits for this tick (with key repeat applied).
    pub nav: u32,
    /// A character typed this tick (one per tick; the rest wait).
    pub ch: Option<char>,
    /// Wheel notches since the last tick; positive scrolls content down.
    pub wheel: f32,
}

/// What the window loop needs from a UI application.
pub trait UiApp {
    /// The UI's size in UI units. The window shows it at a whole-number scale.
    fn size(&self) -> [f32; 2];
    /// Advances the application by one fixed tick.
    fn tick(&mut self, input: &UiTickInput);
    /// What to draw now. Reads only.
    fn frame(&self) -> UiFrame;
}

#[derive(Debug)]
pub struct UiRun {
    pub frames: u64,
    pub ticks: u64,
    /// With `max_frames`: the last frame, rendered offscreen and read back
    /// (swapchain textures cannot be downloaded).
    pub last_frame: Option<Image>,
}

/// Ticks a key is held before it starts repeating, and the ticks between
/// repeats after that.
pub const REPEAT_DELAY: u32 = 24;
pub const REPEAT_EVERY: u32 = 3;

/// Turns held keys into `NAV_*` bits, with repeat for the keys where it
/// helps. A bit is set on the tick a key goes down and again on each repeat,
/// and clear in between, so the UI (which acts on the edge of a bit) sees
/// one press per repeat.
#[derive(Default)]
pub struct NavSampler {
    held: BTreeMap<u32, u32>,
}

impl NavSampler {
    /// `down` lists `(is the key held, its bit, does it repeat)`.
    pub fn sample(&mut self, down: &[(bool, u32, bool)]) -> u32 {
        let mut bits = 0;
        for &(is_down, bit, repeat) in down {
            let ticks = self.held.entry(bit).or_insert(0);
            if is_down {
                let fire = *ticks == 0
                    || (repeat
                        && *ticks >= REPEAT_DELAY
                        && (*ticks - REPEAT_DELAY).is_multiple_of(REPEAT_EVERY));
                *ticks += 1;
                if fire {
                    bits |= bit;
                }
            } else {
                *ticks = 0;
            }
        }
        bits
    }
}

fn keys_down(keys: &KeyboardState) -> Vec<(bool, u32, bool)> {
    let key = |s: Scancode| keys.is_scancode_pressed(s);
    let shift = key(Scancode::LShift) || key(Scancode::RShift);
    vec![
        (key(Scancode::Tab) && !shift, NAV_NEXT, true),
        (key(Scancode::Tab) && shift, NAV_PREV, true),
        (
            key(Scancode::Return) || key(Scancode::KpEnter),
            NAV_ACCEPT,
            false,
        ),
        (key(Scancode::Left), NAV_LEFT, true),
        (key(Scancode::Right), NAV_RIGHT, true),
        (key(Scancode::Up), NAV_UP, true),
        (key(Scancode::Down), NAV_DOWN, true),
        (key(Scancode::Backspace), NAV_BACKSPACE, true),
        (key(Scancode::Delete), NAV_DELETE, true),
        (key(Scancode::Home), NAV_HOME, false),
        (key(Scancode::End), NAV_END, false),
        (key(Scancode::PageUp), NAV_PAGE_UP, true),
        (key(Scancode::PageDown), NAV_PAGE_DOWN, true),
    ]
}

/// Runs `app` in a window until it is closed or Escape is pressed (or
/// `options.max_frames` frames were drawn). Needs the GPU renderer.
pub fn run_ui_windowed<A: UiApp>(app: &mut A, options: &WindowOptions) -> Result<UiRun, String> {
    let sdl = sdl3::init().map_err(|e| e.to_string())?;
    let video = sdl.video().map_err(|e| e.to_string())?;
    let mut builder = video.window(&options.title, options.size.0, options.size.1);
    builder.position_centered().resizable();
    if options.hidden {
        builder.hidden();
    }
    let window = builder.build().map_err(|e| e.to_string())?;
    let mut gpu = GpuRenderer::new(window.clone())
        .map_err(|e| format!("the UI window needs the GPU renderer: {e}"))?;
    video.text_input().start(&window);
    let mut events = sdl.event_pump().map_err(|e| e.to_string())?;

    let font = BitmapFont::builtin();
    let tick_len = Duration::from_secs_f32(TICK_DT);
    let (mut last, mut accumulator) = (Instant::now(), Duration::ZERO);
    let (mut frames, mut ticks) = (0u64, 0u64);
    // Mouse position in window coordinates, whether it is inside, the
    // button, the wheel since the last tick, and typed characters waiting.
    let (mut mouse, mut inside, mut down) = ([0.0f32; 2], false, false);
    let (mut wheel, mut typed) = (0.0f32, VecDeque::<char>::new());
    let mut nav = NavSampler::default();
    let mut last_frame = None;

    'run: loop {
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'run,
                Event::MouseMotion { x, y, .. } => {
                    mouse = [x, y];
                    inside = true;
                }
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    mouse = [x, y];
                    inside = true;
                    down = true;
                }
                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    ..
                } => down = false,
                Event::MouseWheel { y, .. } => wheel -= y,
                Event::TextInput { text, .. } => typed.extend(text.chars()),
                Event::Window {
                    win_event: WindowEvent::MouseLeave,
                    ..
                } => inside = false,
                _ => {}
            }
        }
        let now = Instant::now();
        if options.max_frames.is_some() {
            accumulator = tick_len;
        } else {
            accumulator += (now - last).min(Duration::from_millis(250));
        }
        last = now;
        let (pw, ph) = window.size_in_pixels();
        let (ww, wh) = window.size();
        let viewport = [pw as f32, ph as f32];
        while accumulator >= tick_len {
            let (scale, origin) = ui_view(viewport, app.size());
            // Window coordinates to pixels (they differ on a scaled display),
            // then to UI units.
            let px = [
                mouse[0] * pw as f32 / ww.max(1) as f32,
                mouse[1] * ph as f32 / wh.max(1) as f32,
            ];
            let input = UiTickInput {
                pointer: inside.then(|| [(px[0] - origin[0]) / scale, (px[1] - origin[1]) / scale]),
                down,
                nav: nav.sample(&keys_down(&events.keyboard_state())),
                ch: typed.pop_front(),
                wheel: std::mem::take(&mut wheel),
            };
            app.tick(&input);
            ticks += 1;
            accumulator -= tick_len;
        }
        let frame = app.frame();
        gpu.draw_ui(&frame)?;
        frames += 1;
        if let Some(max) = options.max_frames
            && frames >= max
        {
            let batch = compose_ui(&frame, viewport, &font);
            last_frame = Some(gpu.render_to_image(&batch, [pw, ph])?);
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(UiRun {
        frames,
        ticks,
        last_frame,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u32 = 1;
    const B: u32 = 2;

    #[test]
    fn a_held_key_fires_once_then_repeats_after_the_delay() {
        let mut nav = NavSampler::default();
        let mut fired = vec![];
        for tick in 0..40 {
            if nav.sample(&[(true, A, true)]) & A != 0 {
                fired.push(tick);
            }
        }
        assert_eq!(fired, vec![0, 24, 27, 30, 33, 36, 39]);
    }

    #[test]
    fn keys_that_do_not_repeat_fire_once_per_press() {
        let mut nav = NavSampler::default();
        let mut fired = 0;
        for _ in 0..100 {
            fired += (nav.sample(&[(true, A, false)]) & A != 0) as u32;
        }
        assert_eq!(fired, 1);
        // Released and pressed again: a new press.
        nav.sample(&[(false, A, false)]);
        assert_eq!(nav.sample(&[(true, A, false)]) & A, A);
    }

    #[test]
    fn the_bit_is_clear_between_repeats_so_the_ui_sees_separate_edges() {
        let mut nav = NavSampler::default();
        let bits: Vec<u32> = (0..30).map(|_| nav.sample(&[(true, B, true)])).collect();
        assert_eq!(bits[0], B);
        assert!(bits[1..24].iter().all(|&b| b == 0));
        assert_eq!(bits[24], B);
        assert_eq!(bits[25], 0, "cleared the tick after a repeat");
    }
}
