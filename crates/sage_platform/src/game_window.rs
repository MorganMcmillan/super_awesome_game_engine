//! A window for a game that draws itself as a [`UiFrame`] (a draw list in
//! whole-number units, y down): a fixed-tick loop with recorded button input,
//! sound cues and the GPU renderer.
//!
//! This is `run_windowed` for games that are not Pong: the same split (wall
//! clock decides how many ticks run, never what a tick computes), but the
//! simulation says what to draw with sprites, text and shapes instead of
//! `DrawKind` rectangles, and the view is the game's own size, shown at the
//! largest whole-number scale (`sprite::ui_view`), so pixels stay crisp.
//!
//! GPU only, like the UI window; [`render_frame`] is the CPU reference for
//! screenshots and tests on a machine without one.

use std::time::{Duration, Instant};

use sage_assets::{BitmapFont, Image};
use sage_core::host::UiFrame;
use sage_core::{InputRecording, Simulation, TICK_DT};
use sdl3::event::Event;
use sdl3::gamepad::{Axis, Button, Gamepad};
use sdl3::keyboard::{KeyboardState, Keycode, Scancode};

use crate::gpu::GpuRenderer;
use crate::{InputMap, InputSource, WindowOptions};

/// A simulation that draws itself as a UI draw list.
pub trait GameApp: Simulation {
    /// What to draw now. Reads only.
    fn frame(&self) -> UiFrame;
    /// Image assets the frame draws, by `AssetId`; read once when the window
    /// opens.
    fn images(&self) -> Vec<(u64, Image)> {
        vec![]
    }
    /// Sound cues (names) that happened during the last `step`. The window
    /// loop calls this after every step. The default is silence.
    fn take_cues(&mut self) -> Vec<String> {
        vec![]
    }
}

#[derive(Debug)]
pub struct GameRun {
    pub recording: InputRecording,
    pub frames: u64,
    /// Sound cues the simulation emitted, and how many had a registered sound.
    pub cues_emitted: u64,
    pub cues_played: u64,
    /// Whether an audio device was open.
    pub audio: bool,
    /// With `max_frames`: the last frame, rendered offscreen and read back
    /// (swapchain textures cannot be downloaded).
    pub last_frame: Option<Image>,
}

/// Live keyboard and gamepad state for `InputMap::sample`.
struct Live<'a> {
    keys: KeyboardState<'a>,
    pads: &'a [Gamepad],
}

impl InputSource for Live<'_> {
    fn key(&self, key: Scancode) -> bool {
        self.keys.is_scancode_pressed(key)
    }
    fn pad_button(&self, button: Button) -> bool {
        self.pads.iter().any(|p| p.button(button))
    }
    fn pad_axis(&self, axis: Axis) -> f32 {
        self.pads
            .iter()
            .map(|p| f32::from(p.axis(axis)) / 32767.0)
            .fold(0.0, |a: f32, b| if b.abs() > a.abs() { b } else { a })
    }
}

/// The background a frame is drawn over when the game covers less than all of
/// the window.
pub const CLEAR: [u8; 4] = [8, 8, 12, 255];

/// Renders a frame into `size` pixels with the CPU reference rasterizer (the
/// same batch the GPU draws).
pub fn render_frame(frame: &UiFrame, images: &[(u64, Image)], size: [u32; 2]) -> Image {
    crate::sprite::render_ui(frame, &BitmapFont::builtin(), images, CLEAR, size)
}

/// Runs `sim` in a window at the fixed tick rate, recording input per tick,
/// until the window closes or Escape is pressed (or `options.max_frames`
/// frames were drawn). `seed` is written into the recording so it can be
/// replayed headless. Needs the GPU renderer.
pub fn run_game_windowed<S: GameApp>(
    sim: &mut S,
    seed: u64,
    input_map: &InputMap,
    options: &WindowOptions,
) -> Result<GameRun, String> {
    crate::prepare_video();
    let sdl = sdl3::init().map_err(|e| e.to_string())?;
    let video = sdl.video().map_err(|e| e.to_string())?;
    let mut builder = video.window(&options.title, options.size.0, options.size.1);
    builder.position_centered().resizable();
    if options.hidden {
        builder.hidden();
    }
    let window = builder.build().map_err(|e| e.to_string())?;
    let mut gpu = GpuRenderer::new(window.clone())
        .map_err(|e| format!("this game needs the GPU renderer: {e}"))?;
    for (id, image) in sim.images() {
        gpu.register_ui_image(id, &image)?;
    }
    let mut events = sdl.event_pump().map_err(|e| e.to_string())?;
    let pads_subsystem = sdl.gamepad().map_err(|e| e.to_string())?;
    let mut pads: Vec<Gamepad> = pads_subsystem
        .gamepads()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| pads_subsystem.open(id).ok())
        .collect();
    let mut audio = None;
    if options.audio {
        match crate::audio::AudioOut::open(&sdl) {
            Ok(mut out) => {
                for (cue, sound) in &options.sounds {
                    out.register(cue, sound.clone());
                }
                audio = Some(out);
            }
            Err(e) => eprintln!("note: no audio ({e}); running silent"),
        }
    }
    let (mut cues_emitted, mut cues_played) = (0u64, 0u64);

    let mut recording = InputRecording::new(seed);
    let tick_len = Duration::from_secs_f32(TICK_DT);
    let (mut last, mut accumulator) = (Instant::now(), Duration::ZERO);
    let mut frames = 0u64;
    let mut last_frame = None;

    'run: loop {
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'run,
                Event::GamepadAdded { which, .. } => {
                    if let Ok(pad) = pads_subsystem.open(which) {
                        pads.push(pad);
                    }
                }
                Event::GamepadRemoved { which, .. } => {
                    pads.retain(|p| p.id().is_ok_and(|id| id != which));
                }
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
            let live = Live {
                keys: events.keyboard_state(),
                pads: &pads,
            };
            let input = input_map.sample(&live);
            recording.record(sim.tick(), input);
            sim.step(input);
            for cue in sim.take_cues() {
                cues_emitted += 1;
                if let Some(out) = audio.as_mut()
                    && out.cue(&cue)
                {
                    cues_played += 1;
                }
            }
            accumulator -= tick_len;
        }
        if let Some(out) = audio.as_mut() {
            out.pump()?;
        }
        let frame = sim.frame();
        gpu.draw_ui(&frame)?;
        frames += 1;
        if let Some(max) = options.max_frames
            && frames >= max
        {
            let (pw, ph) = window.size_in_pixels();
            let batch = gpu.compose_ui_batch(&frame, [pw as f32, ph as f32])?;
            last_frame = Some(gpu.render_to_image(&batch, [pw, ph])?);
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    // Make the recording cover every tick that ran, even trailing idle ones.
    if sim.tick() > 0 {
        let last_tick = sim.tick() - 1;
        recording.record(last_tick, recording.get(last_tick));
    }
    Ok(GameRun {
        recording,
        frames,
        cues_emitted,
        cues_played,
        audio: audio.is_some(),
        last_frame,
    })
}
