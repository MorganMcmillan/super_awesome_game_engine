//! Presentation: window, input, drawing and sound on SDL3.
//!
//! Reads simulation state only through `sage_core::host::Frame` and feeds the
//! simulation only `TickInput`, which is recorded per tick. Wall-clock time
//! lives here and decides *how many* ticks to run per frame, never what a
//! tick computes, so a recorded run replays identically headless.
//!
//! Milestone 1 draws with the GPU renderer (`gpu`: sprite batches through
//! SDL's GPU API, pixel-perfect mode) and keeps `SdlRenderer`, SDL's simple
//! Render API, as the fallback for machines without Vulkan. Sound is a
//! software mixer (`mixer`) on one SDL audio stream (`audio`). Input is
//! mapped to game bits by `input::InputMap`.

pub mod a11y;
pub mod audio;
pub mod game_window;
pub mod gpu;
pub mod input;
pub mod media;
pub mod mixer;
pub mod sprite;
pub mod ui_window;

use std::rc::Rc;
use std::time::{Duration, Instant};

use sage_assets::{Image, Sound};
use sage_core::host::{DrawKind, Frame};
use sage_core::{InputRecording, Simulation, TICK_DT};
use sdl3::event::Event;
use sdl3::gamepad::{Axis, Button, Gamepad};
use sdl3::keyboard::{KeyboardState, Keycode, Scancode};
use sdl3::pixels::Color;
use sdl3::render::FRect;
use sdl3::render::WindowCanvas;

pub use input::{Binding, InputMap, InputSource};
pub use sdl3::gamepad::{Axis as PadAxis, Button as PadButton};
pub use sdl3::keyboard::Scancode as Key;

/// Draws a frame. Implementations must not affect the simulation.
pub trait Renderer {
    fn draw(&mut self, frame: &Frame) -> Result<(), String>;
}

/// A simulation that can describe itself to presentation.
pub trait Presentable: Simulation {
    fn frame(&self) -> Frame;

    /// Sound cues (names) that happened during the last `step`. The window
    /// loop calls this after every step, so none are lost when several ticks
    /// run in one frame. The default is silence.
    fn take_cues(&mut self) -> Vec<String> {
        vec![]
    }
}

/// The fill color of each kind, as RGB bytes. Both renderers use it.
pub fn kind_rgb(kind: DrawKind) -> [u8; 3] {
    match kind {
        DrawKind::Court => [16, 20, 28],
        DrawKind::Wall => [120, 130, 150],
        DrawKind::Goal => [60, 30, 40],
        DrawKind::Paddle => [235, 235, 240],
        DrawKind::Ball => [255, 200, 60],
    }
}

pub fn color_of(kind: DrawKind) -> Color {
    let [r, g, b] = kind_rgb(kind);
    Color::RGB(r, g, b)
}

const KINDS: [DrawKind; 5] = [
    DrawKind::Court,
    DrawKind::Wall,
    DrawKind::Goal,
    DrawKind::Paddle,
    DrawKind::Ball,
];

/// Counts pixels of each `DrawKind` color in an RGBA image. Slow; for smoke tests.
pub fn count_kind_pixels(img: &Image) -> Vec<(DrawKind, usize)> {
    KINDS
        .iter()
        .map(|&k| {
            let c = kind_rgb(k);
            let n = img
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0] == c[0] && p[1] == c[1] && p[2] == c[2])
                .count();
            (k, n)
        })
        .collect()
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
        Ok(surface.with_lock(|bytes| {
            let rgba: Vec<u8> = bytes
                .chunks(pitch)
                .flat_map(|row| row[..w * 4].iter().copied())
                .collect();
            let img = Image {
                width: w as u32,
                height: surface.height(),
                rgba,
            };
            count_kind_pixels(&img)
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

/// Which renderer draws the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RendererChoice {
    /// The GPU renderer, falling back to the Render API when GPU setup fails.
    Auto,
    /// The GPU renderer; setup failure is an error.
    Gpu,
    /// SDL's simple Render API.
    Sdl,
}

/// Picks a video driver that works before SDL starts. On Wayland desktops
/// whose GNOME settings schema lacks a key SDL's libdecor GTK plugin reads
/// (`org.gnome.settings-daemon.plugins.xsettings`, `antialiasing`), the plugin
/// aborts the whole process. When an X server is reachable (XWayland) and the
/// person has not chosen a driver, use it, with Wayland as the fallback;
/// `SDL_VIDEODRIVER=wayland` still forces Wayland. Call before `sdl3::init`.
pub fn prepare_video() {
    let chosen = std::env::var_os("SDL_VIDEODRIVER").is_some();
    if !chosen && std::env::var_os("DISPLAY").is_some() {
        sdl3::hint::set("SDL_VIDEO_DRIVER", "x11,wayland");
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
    pub renderer: RendererChoice,
    pub scale_mode: sprite::ScaleMode,
    /// Images drawn instead of the solid color for a `DrawKind` (GPU renderer).
    pub sprites: Vec<(DrawKind, Image)>,
    /// Sounds by cue name, played when the simulation emits that cue.
    pub sounds: Vec<(String, Rc<Sound>)>,
    /// Open an audio device. Failure to open one is reported and the run is silent.
    pub audio: bool,
}

impl WindowOptions {
    pub fn new(title: impl Into<String>, size: (u32, u32)) -> WindowOptions {
        WindowOptions {
            title: title.into(),
            size,
            max_frames: None,
            hidden: false,
            renderer: RendererChoice::Auto,
            scale_mode: sprite::ScaleMode::PixelPerfect,
            sprites: vec![],
            sounds: vec![],
            audio: true,
        }
    }
}

#[derive(Debug)]
pub struct WindowRun {
    pub recording: InputRecording,
    pub frames: u64,
    /// Pixel counts from the last frame, when `max_frames` was set.
    pub last_frame_pixels: Option<Vec<(DrawKind, usize)>>,
    /// Which renderer ran: `"gpu"` or `"sdl"`.
    pub renderer: &'static str,
    /// Sound cues the simulation emitted, and how many had a registered sound.
    pub cues_emitted: u64,
    pub cues_played: u64,
    /// Whether an audio device was open.
    pub audio: bool,
}

enum Backend {
    Gpu(Box<gpu::GpuRenderer>),
    Sdl(SdlRenderer),
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

/// Runs `sim` in a window at the fixed tick rate, recording input per tick.
/// `seed` is written into the recording so it can be replayed headless.
pub fn run_windowed<S: Presentable>(
    sim: &mut S,
    seed: u64,
    input_map: &InputMap,
    options: &WindowOptions,
) -> Result<WindowRun, String> {
    prepare_video();
    let sdl = sdl3::init().map_err(|e| e.to_string())?;
    let video = sdl.video().map_err(|e| e.to_string())?;
    let mut builder = video.window(&options.title, options.size.0, options.size.1);
    builder.position_centered().resizable();
    if options.hidden {
        builder.hidden();
    }
    let window = builder.build().map_err(|e| e.to_string())?;
    let gpu = |window: &sdl3::video::Window| -> Result<Backend, String> {
        let mut r = gpu::GpuRenderer::new(window.clone())?;
        r.mode = options.scale_mode;
        for (kind, image) in &options.sprites {
            let tex = r.add_texture(image)?;
            r.skin.sprites.push((*kind, tex));
        }
        Ok(Backend::Gpu(Box::new(r)))
    };
    let mut backend = match options.renderer {
        RendererChoice::Sdl => Backend::Sdl(SdlRenderer::new(window.clone().into_canvas())),
        RendererChoice::Gpu => gpu(&window)?,
        RendererChoice::Auto => gpu(&window).unwrap_or_else(|e| {
            eprintln!("note: GPU renderer unavailable ({e}); using the SDL Render API");
            Backend::Sdl(SdlRenderer::new(window.clone().into_canvas()))
        }),
    };
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
        match audio::AudioOut::open(&sdl) {
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
        match &mut backend {
            Backend::Gpu(r) => r.draw(&frame)?,
            Backend::Sdl(r) => r.draw(&frame)?,
        }
        frames += 1;
        if let Some(max) = options.max_frames
            && frames >= max
        {
            last_frame_pixels = Some(match &mut backend {
                Backend::Gpu(r) => {
                    // Read back through an offscreen render of the same frame:
                    // swapchain textures cannot be downloaded.
                    let (w, h) = (options.size.0, options.size.1);
                    let batch = sprite::compose(
                        &frame,
                        [w as f32, h as f32],
                        options.scale_mode,
                        &sage_assets::BitmapFont::builtin(),
                        &r.skin,
                    );
                    count_kind_pixels(&r.render_to_image(&batch, [w, h])?)
                }
                Backend::Sdl(r) => {
                    let px = r.count_colored_pixels()?;
                    r.present();
                    px
                }
            });
            break;
        }
        if let Backend::Sdl(r) = &mut backend {
            r.present();
        }
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
        renderer: match backend {
            Backend::Gpu(_) => "gpu",
            Backend::Sdl(_) => "sdl",
        },
        cues_emitted,
        cues_played,
        audio: audio.is_some(),
    })
}
