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

use sage_assets::Image;
use sage_core::TICK_DT;
use sage_core::host::{
    CHORD_ALT, CHORD_CTRL, CHORD_F1, CHORD_SHIFT, NAV_ACCEPT, NAV_ALT, NAV_BACKSPACE, NAV_CANCEL,
    NAV_COPY, NAV_CUT, NAV_DELETE, NAV_DOWN, NAV_END, NAV_HOME, NAV_LEFT, NAV_NEXT, NAV_PAGE_DOWN,
    NAV_PAGE_UP, NAV_PASTE, NAV_PREV, NAV_REDO, NAV_RIGHT, NAV_SELECT_ALL, NAV_SHIFT, NAV_UNDO,
    NAV_UP, NAV_WORD, UiFrame,
};
use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::{KeyboardState, Keycode, Scancode};
use sdl3::mouse::MouseButton;

use crate::WindowOptions;
use crate::gpu::GpuRenderer;
use crate::sprite::ui_view;

/// One tick of UI input, as the window loop samples it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiTickInput {
    /// The pointer in UI units, or `None` when it is outside the window.
    pub pointer: Option<[f32; 2]>,
    /// The left mouse button is held.
    pub down: bool,
    /// The right mouse button is held.
    pub secondary: bool,
    /// The middle mouse button is held.
    pub middle: bool,
    /// A key chord pressed this tick: the key (upper-case ASCII or
    /// `CHORD_F1 + n - 1`) and its `CHORD_*` modifiers.
    pub chord: Option<[u32; 2]>,
    /// `NAV_*` bits for this tick (with key repeat applied).
    pub nav: u32,
    /// A character typed this tick (one per tick; the rest wait).
    pub ch: Option<char>,
    /// Wheel notches since the last tick; positive scrolls content down.
    pub wheel: f32,
    /// The system clipboard's text, read on the tick a paste is asked for.
    pub paste: Option<String>,
    /// Text an input method is composing, not yet committed (empty for none).
    /// Not recorded in `sage-input`: it is shown, never acted on.
    pub composing: String,
}

impl UiTickInput {
    /// Writes this tick into a recording: the mouse buttons as `buttons` and
    /// the rest as the pointer stream. The clipboard text is not recorded.
    pub fn record_into(&self, rec: &mut sage_core::input::InputRecording, tick: u64) {
        let buttons = self.down as u32 | (self.secondary as u32) << 1 | (self.middle as u32) << 2;
        rec.record(tick, sage_core::input::TickInput { buttons });
        rec.record_pointer(
            tick,
            sage_core::input::PointerInput::new(
                self.pointer,
                self.nav,
                self.wheel,
                self.chord,
                self.ch,
            ),
        );
    }

    /// The input a recording holds for `tick`.
    pub fn from_recording(rec: &sage_core::input::InputRecording, tick: u64) -> UiTickInput {
        let (b, p) = (rec.get(tick).buttons, rec.get_pointer(tick));
        UiTickInput {
            pointer: p.pointer(),
            down: b & 1 != 0,
            secondary: b & 2 != 0,
            middle: b & 4 != 0,
            chord: p.chord,
            nav: p.mods,
            ch: p.typed(),
            wheel: p.wheel_notches(),
            paste: None,
            composing: String::new(),
        }
    }
}

/// A request for the operating system's file dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileRequest {
    /// Save instead of open.
    pub save: bool,
}

/// What the window loop needs from a UI application.
pub trait UiApp {
    /// The UI's size in UI units. The window shows it at a whole-number scale.
    fn size(&self) -> [f32; 2];
    /// Advances the application by one fixed tick.
    fn tick(&mut self, input: &UiTickInput);
    /// What to draw now. Reads only.
    fn frame(&self) -> UiFrame;
    /// Image assets the UI draws, by `AssetId`; read once when the window
    /// opens.
    fn images(&self) -> Vec<(u64, Image)> {
        vec![]
    }
    /// Text a copy or cut put on the clipboard since the last call; the window
    /// publishes it to the system.
    fn take_clipboard(&mut self) -> Option<String> {
        None
    }
    /// Interleaved stereo samples (`frames` of them, at
    /// [`crate::audio::OUT_RATE`]) for the audio device, if this app makes
    /// sound. The window asks when the device queue runs low.
    fn mix_audio(&mut self, _frames: usize) -> Option<Vec<f32>> {
        None
    }
    /// A file dialog the UI wants shown, if any. The window shows it and
    /// reports the answer through [`UiApp::file_chosen`].
    fn take_file_request(&mut self) -> Option<FileRequest> {
        None
    }
    /// The answer to a file dialog: the chosen path, or `None` if it was
    /// cancelled.
    fn file_chosen(&mut self, _path: Option<String>) {}
    /// What is on screen for assistive technology: a full tree update. Called
    /// each tick, but only while something is listening.
    fn access_tree(&self) -> accesskit::TreeUpdate {
        crate::a11y::empty_update()
    }
    /// An action assistive technology asked for (focus, click).
    fn access_action(&mut self, _request: accesskit::ActionRequest) {}
    /// The text face to draw with, when it is not the built-in one. Called
    /// once; `scale` is the window's whole-number scale (device pixels per UI
    /// unit), for a face that is rasterized at the size it is shown.
    fn font(&self, _scale: u32) -> Option<sage_assets::BitmapFont> {
        None
    }
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
    let ctrl = key(Scancode::LCtrl) || key(Scancode::RCtrl);
    vec![
        (key(Scancode::Tab) && !shift, NAV_NEXT, true),
        (key(Scancode::Tab) && shift, NAV_PREV, true),
        (
            key(Scancode::Return) || key(Scancode::KpEnter),
            NAV_ACCEPT,
            false,
        ),
        (key(Scancode::Escape), NAV_CANCEL, false),
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
        (ctrl && key(Scancode::A), NAV_SELECT_ALL, false),
        (ctrl && key(Scancode::C), NAV_COPY, false),
        (ctrl && key(Scancode::X), NAV_CUT, false),
        (ctrl && key(Scancode::V), NAV_PASTE, true),
        (ctrl && !shift && key(Scancode::Z), NAV_UNDO, true),
        (
            ctrl && (key(Scancode::Y) || shift && key(Scancode::Z)),
            NAV_REDO,
            true,
        ),
    ]
}

/// The modifier bits, held for as long as the keys are: Shift extends a
/// selection, Ctrl moves by word.
pub fn modifier_bits(keys: &KeyboardState) -> u32 {
    let key = |s: Scancode| keys.is_scancode_pressed(s);
    let mut bits = 0;
    if key(Scancode::LShift) || key(Scancode::RShift) {
        bits |= NAV_SHIFT;
    }
    if key(Scancode::LCtrl) || key(Scancode::RCtrl) {
        bits |= NAV_WORD;
    }
    if key(Scancode::LAlt) || key(Scancode::RAlt) {
        bits |= NAV_ALT;
    }
    bits
}

/// The chord a key press makes, if it is one: a letter or digit with Ctrl or
/// Alt held, an arrow key with Ctrl or Alt, Delete, or a function key with any
/// modifiers.
pub fn chord_of(code: Keycode, keymod: sdl3::keyboard::Mod) -> Option<[u32; 2]> {
    use sdl3::keyboard::Mod;
    let mut mods = 0;
    if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) {
        mods |= CHORD_CTRL;
    }
    if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
        mods |= CHORD_SHIFT;
    }
    if keymod.intersects(Mod::LALTMOD | Mod::RALTMOD) {
        mods |= CHORD_ALT;
    }
    let function = [
        Keycode::F1,
        Keycode::F2,
        Keycode::F3,
        Keycode::F4,
        Keycode::F5,
        Keycode::F6,
        Keycode::F7,
        Keycode::F8,
        Keycode::F9,
        Keycode::F10,
        Keycode::F11,
        Keycode::F12,
    ]
    .iter()
    .position(|k| *k == code);
    if let Some(n) = function {
        return Some([CHORD_F1 + n as u32, mods]);
    }
    // Arrows with Ctrl or Alt, and Delete with anything (a text field also
    // sees the Delete, so a script that binds it checks where focus is).
    let named = match code {
        Keycode::Up => Some(sage_core::host::CHORD_UP),
        Keycode::Down => Some(sage_core::host::CHORD_DOWN),
        Keycode::Left => Some(sage_core::host::CHORD_LEFT),
        Keycode::Right => Some(sage_core::host::CHORD_RIGHT),
        Keycode::Delete => Some(sage_core::host::CHORD_DELETE),
        _ => None,
    };
    if let Some(k) = named
        && (k == sage_core::host::CHORD_DELETE || mods & (CHORD_CTRL | CHORD_ALT) != 0)
    {
        return Some([k, mods]);
    }
    let raw: u32 = code as u32;
    let ascii = (0x20..0x7f)
        .contains(&raw)
        .then(|| (raw as u8).to_ascii_uppercase());
    match ascii {
        Some(c) if c.is_ascii_alphanumeric() && mods & (CHORD_CTRL | CHORD_ALT) != 0 => {
            Some([c as u32, mods])
        }
        _ => None,
    }
}

/// What the window's events add up to between ticks: where the mouse is,
/// the button, the wheel and the typed characters. Kept apart from the loop
/// so the translation can be tested with plain `Event` values.
#[derive(Default)]
pub struct UiEvents {
    /// Mouse position in window coordinates.
    pub mouse: [f32; 2],
    pub inside: bool,
    pub down: bool,
    pub secondary: bool,
    pub middle: bool,
    chord: Option<[u32; 2]>,
    wheel: f32,
    /// Sideways wheel notches (a touchpad's horizontal swipe); positive is
    /// towards the right.
    wheel_x: f32,
    typed: VecDeque<char>,
    composing: String,
}

impl UiEvents {
    /// Takes one event in. Returns true if it asks to quit (the window was
    /// closed, or Escape).
    pub fn handle(&mut self, event: &Event) -> bool {
        match event {
            // Escape belongs to the UI (it closes popups); a window is closed
            // with its close button, or Ctrl+Q.
            Event::Quit { .. } => return true,
            Event::KeyDown {
                keycode: Some(Keycode::Q),
                keymod,
                ..
            } if keymod
                .intersects(sdl3::keyboard::Mod::LCTRLMOD | sdl3::keyboard::Mod::RCTRLMOD) =>
            {
                return true;
            }
            Event::MouseMotion { x, y, .. } => {
                self.mouse = [*x, *y];
                self.inside = true;
            }
            Event::MouseButtonDown {
                mouse_btn: MouseButton::Left,
                x,
                y,
                ..
            } => {
                self.mouse = [*x, *y];
                self.inside = true;
                self.down = true;
            }
            Event::MouseButtonUp {
                mouse_btn: MouseButton::Left,
                ..
            } => self.down = false,
            Event::MouseButtonDown {
                mouse_btn: MouseButton::Right,
                x,
                y,
                ..
            } => {
                self.mouse = [*x, *y];
                self.inside = true;
                self.secondary = true;
            }
            Event::MouseButtonUp {
                mouse_btn: MouseButton::Right,
                ..
            } => self.secondary = false,
            Event::MouseButtonDown {
                mouse_btn: MouseButton::Middle,
                x,
                y,
                ..
            } => {
                self.mouse = [*x, *y];
                self.inside = true;
                self.middle = true;
            }
            Event::MouseButtonUp {
                mouse_btn: MouseButton::Middle,
                ..
            } => self.middle = false,
            Event::KeyDown {
                keycode: Some(code),
                keymod,
                repeat: false,
                ..
            } => {
                if let Some(chord) = chord_of(*code, *keymod) {
                    self.chord = Some(chord);
                }
            }
            // SDL's wheel is positive away from the user (scroll up); the UI
            // counts positive as content moving down.
            // A sideways swipe is kept apart: `tick` turns it into the wheel
            // with Shift held, which is how the UI scrolls across the flow.
            Event::MouseWheel { x, y, .. } => {
                self.wheel -= *y;
                self.wheel_x += *x;
            }
            Event::TextInput { text, .. } => {
                self.typed.extend(text.chars());
                // What was being composed is now committed.
                self.composing.clear();
            }
            Event::TextEditing { text, .. } => self.composing = text.clone(),
            Event::Window {
                win_event: WindowEvent::MouseLeave,
                ..
            } => self.inside = false,
            _ => {}
        }
        false
    }

    /// The input for one tick: the pointer through `to_ui` (window
    /// coordinates to UI units), one typed character, and the wheel since
    /// the last tick. `nav` is the sampled `NAV_*` bits.
    pub fn tick(&mut self, nav: u32, to_ui: impl Fn([f32; 2]) -> [f32; 2]) -> UiTickInput {
        // One axis per tick, whichever moved more, so the recorded input
        // (one wheel number and the Shift bit) holds it without a new format.
        let (wheel, wheel_x) = (
            std::mem::take(&mut self.wheel),
            std::mem::take(&mut self.wheel_x),
        );
        let (wheel, nav) = if wheel_x.abs() > wheel.abs() {
            (wheel_x, nav | NAV_SHIFT)
        } else {
            (wheel, nav)
        };
        UiTickInput {
            pointer: self.inside.then(|| to_ui(self.mouse)),
            down: self.down,
            secondary: self.secondary,
            middle: self.middle,
            chord: self.chord.take(),
            nav,
            ch: self.typed.pop_front(),
            wheel,
            paste: None,
            composing: self.composing.clone(),
        }
    }
}

/// Shows the system's open or save dialog over `window`; the chosen path (or
/// `None` for a cancel or an error) is sent on `answer`.
fn show_file_dialog(
    request: FileRequest,
    window: &sdl3::video::Window,
    answer: std::sync::mpsc::Sender<Option<String>>,
) {
    let callback: sdl3::dialog::DialogCallback = Box::new(move |result, _filter| {
        if let Err(e) = &result
            && !matches!(e, sdl3::dialog::DialogError::Canceled)
        {
            eprintln!("file dialog: {e:?}");
        }
        let path = result
            .ok()
            .and_then(|paths| paths.into_iter().next())
            .map(|p| p.to_string_lossy().into_owned());
        let _ = answer.send(path);
    });
    let filters = [sdl3::dialog::DialogFileFilter {
        name: "All files",
        pattern: "*",
    }];
    let none: Option<&std::path::Path> = None;
    let shown = if request.save {
        sdl3::dialog::show_save_file_dialog(&filters, none, window, callback)
    } else {
        sdl3::dialog::show_open_file_dialog(&filters, none, false, window, callback)
    };
    if let Err(e) = shown {
        eprintln!("file dialog could not open: {e:?}");
    }
}

/// Runs `app` in a window until it is closed or Escape is pressed (or
/// `options.max_frames` frames were drawn). Needs the GPU renderer.
pub fn run_ui_windowed<A: UiApp>(app: &mut A, options: &WindowOptions) -> Result<UiRun, String> {
    crate::prepare_video();
    let sdl = sdl3::init().map_err(|e| e.to_string())?;
    let video = sdl.video().map_err(|e| e.to_string())?;
    let mut builder = video.window(&options.title, options.size.0, options.size.1);
    builder.position_centered().resizable();
    if options.hidden {
        builder.hidden();
    }
    // Built hidden so the accessibility adapter can attach before the window
    // is first shown (Windows needs that); shown below unless it should stay
    // hidden.
    builder.hidden();
    let mut window = builder.build().map_err(|e| e.to_string())?;
    let mut a11y = crate::a11y::Accessibility::attach(&window);
    if !options.hidden {
        window.show();
    }
    let mut gpu = GpuRenderer::new(window.clone())
        .map_err(|e| format!("the UI window needs the GPU renderer: {e}"))?;
    for (id, image) in app.images() {
        gpu.register_ui_image(id, &image)?;
    }
    {
        let (pw, ph) = window.size_in_pixels();
        let (scale, _) = ui_view([pw as f32, ph as f32], app.size());
        if let Some(font) = app.font(scale.round().max(1.0) as u32) {
            gpu.set_font(font)?;
        }
    }
    video.text_input().start(&window);
    let clipboard = video.clipboard();
    let mut events = sdl.event_pump().map_err(|e| e.to_string())?;

    let tick_len = Duration::from_secs_f32(TICK_DT);
    let (mut last, mut accumulator) = (Instant::now(), Duration::ZERO);
    let (mut frames, mut ticks) = (0u64, 0u64);
    let mut input_events = UiEvents::default();
    let mut nav = NavSampler::default();
    let mut last_frame = None;
    // File dialogs are asynchronous: the answer arrives on this channel.
    let (dialog_tx, dialog_rx) = std::sync::mpsc::channel::<Option<String>>();
    let mut dialog_open = false;
    // Sound is for a window the user sees: a hidden smoke-test window stays
    // silent, and so does a machine with no audio device.
    let mut audio = if options.max_frames.is_none() {
        crate::audio::AudioOut::open(&sdl).ok()
    } else {
        None
    };

    'run: loop {
        for event in events.poll_iter() {
            if input_events.handle(&event) {
                break 'run;
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
            let to_ui = |m: [f32; 2]| {
                let px = [
                    m[0] * pw as f32 / ww.max(1) as f32,
                    m[1] * ph as f32 / wh.max(1) as f32,
                ];
                [(px[0] - origin[0]) / scale, (px[1] - origin[1]) / scale]
            };
            let keys = events.keyboard_state();
            let bits = nav.sample(&keys_down(&keys)) | modifier_bits(&keys);
            let mut input = input_events.tick(bits, to_ui);
            if bits & NAV_PASTE != 0 {
                input.paste = clipboard.clipboard_text().ok();
            }
            app.tick(&input);
            while let Ok(answer) = dialog_rx.try_recv() {
                dialog_open = false;
                app.file_chosen(answer);
            }
            if let Some(a) = a11y.as_mut() {
                a.update(|| app.access_tree());
                for request in a.actions() {
                    app.access_action(request);
                }
            }
            if !dialog_open && let Some(request) = app.take_file_request() {
                dialog_open = true;
                show_file_dialog(request, &window, dialog_tx.clone());
            }
            if let Some(text) = app.take_clipboard() {
                let _ = clipboard.set_clipboard_text(&text);
            }
            ticks += 1;
            accumulator -= tick_len;
        }
        if let Some(out) = audio.as_mut() {
            let want = out.wanted_frames();
            if want > 0
                && let Some(samples) = app.mix_audio(want)
            {
                out.push(&samples)?;
            }
        }
        let frame = app.frame();
        gpu.draw_ui(&frame)?;
        frames += 1;
        if let Some(max) = options.max_frames
            && frames >= max
        {
            let batch = gpu.compose_ui_batch(&frame, viewport)?;
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

    fn button(down: bool, x: f32, y: f32) -> Event {
        let (timestamp, window_id, which, clicks) = (0, 0, 0, 1);
        if down {
            Event::MouseButtonDown {
                timestamp,
                window_id,
                which,
                mouse_btn: MouseButton::Left,
                clicks,
                x,
                y,
            }
        } else {
            Event::MouseButtonUp {
                timestamp,
                window_id,
                which,
                mouse_btn: MouseButton::Left,
                clicks,
                x,
                y,
            }
        }
    }

    fn motion(x: f32, y: f32) -> Event {
        Event::MouseMotion {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mousestate: sdl3::mouse::MouseState::from_sdl_state(0),
            x,
            y,
            xrel: 0.0,
            yrel: 0.0,
        }
    }

    fn typed(text: &str) -> Event {
        Event::TextInput {
            timestamp: 0,
            window_id: 0,
            text: text.into(),
        }
    }

    #[test]
    fn mouse_events_become_a_pointer_in_ui_units() {
        let mut e = UiEvents::default();
        let to_ui = |m: [f32; 2]| [(m[0] - 10.0) / 2.0, (m[1] - 4.0) / 2.0];
        assert_eq!(
            e.tick(0, to_ui).pointer,
            None,
            "no pointer before the mouse moves"
        );
        e.handle(&motion(30.0, 24.0));
        let t = e.tick(0, to_ui);
        assert_eq!((t.pointer, t.down), (Some([10.0, 10.0]), false));
        assert!(!e.handle(&button(true, 40.0, 24.0)));
        let t = e.tick(0, to_ui);
        assert_eq!(
            (t.pointer, t.down),
            (Some([15.0, 10.0]), true),
            "a press also moves the pointer"
        );
        e.handle(&button(false, 40.0, 24.0));
        assert!(!e.tick(0, to_ui).down);
        // Leaving the window removes the pointer; moving again brings it back.
        e.handle(&Event::Window {
            timestamp: 0,
            window_id: 0,
            win_event: WindowEvent::MouseLeave,
        });
        assert_eq!(e.tick(0, to_ui).pointer, None);
        e.handle(&motion(12.0, 6.0));
        assert_eq!(e.tick(0, to_ui).pointer, Some([1.0, 1.0]));
    }

    #[test]
    fn the_right_button_is_secondary_and_ctrl_q_or_close_quits() {
        let mut e = UiEvents::default();
        let right = Event::MouseButtonDown {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mouse_btn: MouseButton::Right,
            clicks: 1,
            x: 1.0,
            y: 1.0,
        };
        assert!(!e.handle(&right));
        assert!(!e.down, "the right button is not the primary one");
        assert!(e.secondary && e.inside, "it is the secondary button");
        let esc = Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(Keycode::Escape),
            scancode: Some(Scancode::Escape),
            keymod: sdl3::keyboard::Mod::empty(),
            repeat: false,
            which: 0,
            raw: 0,
        };
        assert!(
            !e.handle(&esc),
            "Escape belongs to the UI now, it does not quit"
        );
        let ctrl_q = Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(Keycode::Q),
            scancode: Some(Scancode::Q),
            keymod: sdl3::keyboard::Mod::LCTRLMOD,
            repeat: false,
            which: 0,
            raw: 0,
        };
        assert!(e.handle(&ctrl_q));
        assert!(e.handle(&Event::Quit { timestamp: 0 }));
    }

    #[test]
    fn the_middle_button_and_key_chords_reach_the_tick_once() {
        use sdl3::keyboard::Mod;
        let mut e = UiEvents::default();
        let middle = |down: bool| {
            if down {
                Event::MouseButtonDown {
                    timestamp: 0,
                    window_id: 0,
                    which: 0,
                    mouse_btn: MouseButton::Middle,
                    clicks: 1,
                    x: 2.0,
                    y: 3.0,
                }
            } else {
                Event::MouseButtonUp {
                    timestamp: 0,
                    window_id: 0,
                    which: 0,
                    mouse_btn: MouseButton::Middle,
                    clicks: 1,
                    x: 2.0,
                    y: 3.0,
                }
            }
        };
        e.handle(&middle(true));
        let t = e.tick(0, |m| m);
        assert!(t.middle && !t.down && !t.secondary);
        e.handle(&middle(false));
        assert!(!e.tick(0, |m| m).middle);
        // Ctrl+S is a chord, delivered on one tick only; a bare S is typing.
        let key = |code: Keycode, keymod: Mod, repeat: bool| Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(code),
            scancode: None,
            keymod,
            repeat,
            which: 0,
            raw: 0,
        };
        e.handle(&key(Keycode::S, Mod::empty(), false));
        assert_eq!(e.tick(0, |m| m).chord, None);
        e.handle(&key(Keycode::S, Mod::LCTRLMOD, false));
        assert_eq!(e.tick(0, |m| m).chord, Some([b'S' as u32, CHORD_CTRL]));
        assert_eq!(e.tick(0, |m| m).chord, None, "it fires once");
        e.handle(&key(Keycode::S, Mod::LCTRLMOD, true));
        assert_eq!(
            e.tick(0, |m| m).chord,
            None,
            "a held key does not repeat it"
        );
        // Function keys need no modifier; Alt and Shift add their bits.
        e.handle(&key(Keycode::F5, Mod::empty(), false));
        assert_eq!(e.tick(0, |m| m).chord, Some([CHORD_F1 + 4, 0]));
        e.handle(&key(Keycode::_1, Mod::LALTMOD | Mod::LSHIFTMOD, false));
        assert_eq!(
            e.tick(0, |m| m).chord,
            Some([b'1' as u32, CHORD_ALT | CHORD_SHIFT])
        );
    }

    #[test]
    fn composing_text_shows_until_it_is_committed() {
        let mut e = UiEvents::default();
        let edit = |text: &str| Event::TextEditing {
            timestamp: 0,
            window_id: 0,
            text: text.to_string(),
            start: 0,
            length: text.chars().count() as i32,
        };
        e.handle(&edit("ni"));
        assert_eq!(e.tick(0, |m| m).composing, "ni");
        // It stays while the user keeps composing, even across ticks.
        assert_eq!(e.tick(0, |m| m).composing, "ni");
        e.handle(&edit("nih"));
        assert_eq!(e.tick(0, |m| m).composing, "nih");
        // The committed text arrives as typing and ends the composition.
        e.handle(&typed("\u{4f60}"));
        let t = e.tick(0, |m| m);
        assert_eq!((t.composing.as_str(), t.ch), ("", Some('\u{4f60}')));
        // An empty edit (the user cancelled) clears it too.
        e.handle(&edit("x"));
        e.handle(&edit(""));
        assert_eq!(e.tick(0, |m| m).composing, "");
    }

    #[test]
    fn typed_text_arrives_one_character_per_tick_and_the_wheel_is_consumed_once() {
        let mut e = UiEvents::default();
        let id = |m| m;
        e.handle(&typed("hé"));
        e.handle(&typed("!"));
        let got: Vec<Option<char>> = (0..4).map(|_| e.tick(0, id).ch).collect();
        assert_eq!(got, vec![Some('h'), Some('é'), Some('!'), None]);
        // Two notches away from the user scroll up (negative); one back down.
        let wheel = |y| Event::MouseWheel {
            timestamp: 0,
            window_id: 0,
            which: 0,
            x: 0.0,
            y,
            direction: sdl3::mouse::MouseWheelDirection::Normal,
            mouse_x: 0.0,
            mouse_y: 0.0,
            integer_x: 0,
            integer_y: y as i32,
        };
        e.handle(&wheel(2.0));
        e.handle(&wheel(-0.5));
        assert_eq!(e.tick(0, id).wheel, -1.5);
        assert_eq!(e.tick(0, id).wheel, 0.0, "consumed");
        // A sideways swipe is the wheel with Shift held; the bigger axis wins.
        let swipe = |x, y| Event::MouseWheel {
            timestamp: 0,
            window_id: 0,
            which: 0,
            x,
            y,
            direction: sdl3::mouse::MouseWheelDirection::Normal,
            mouse_x: 0.0,
            mouse_y: 0.0,
            integer_x: 0,
            integer_y: 0,
        };
        e.handle(&swipe(2.0, 0.5));
        let t = e.tick(0, id);
        assert_eq!((t.wheel, t.nav & NAV_SHIFT), (2.0, NAV_SHIFT));
        e.handle(&swipe(0.5, 2.0));
        let t = e.tick(0, id);
        assert_eq!((t.wheel, t.nav & NAV_SHIFT), (-2.0, 0));
    }
}

#[cfg(test)]
mod chord_tests {
    use super::*;
    use sdl3::keyboard::Mod;

    #[test]
    fn arrows_with_alt_or_ctrl_and_delete_are_chords_but_a_bare_arrow_is_navigation() {
        use sage_core::host::{CHORD_ALT, CHORD_CTRL, CHORD_DELETE, CHORD_DOWN, CHORD_UP};
        assert_eq!(
            chord_of(Keycode::Up, Mod::LALTMOD),
            Some([CHORD_UP, CHORD_ALT])
        );
        assert_eq!(
            chord_of(Keycode::Down, Mod::LCTRLMOD),
            Some([CHORD_DOWN, CHORD_CTRL])
        );
        assert_eq!(chord_of(Keycode::Up, Mod::NOMOD), None);
        assert_eq!(
            chord_of(Keycode::Delete, Mod::NOMOD),
            Some([CHORD_DELETE, 0])
        );
    }
}
