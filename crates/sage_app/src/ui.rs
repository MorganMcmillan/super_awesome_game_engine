//! A host for a world that runs the UI kit.
//!
//! [`UiHost`] owns the world, the binding and the scripts, and does the parts
//! of a tick that are the same for every UI application. An application wraps
//! one and implements [`UiHosted`], which gives it the window loop's contract
//! and the gestures its tests are written in.

use std::cell::RefCell;
use std::ffi::CString;

use flecs_ecs::prelude::*;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use sage_assets::{BitmapFont, Image};
use sage_core::StateHasher;
use sage_core::host::{UiDraw, UiFrame};
use sage_core::input::InputRecording;
use sage_ecs_py::Ecs;
use sage_platform::WindowOptions;
use sage_platform::ui_window::{FileRequest, UiApp, UiRun, UiTickInput};
use sage_script::Script;
use sage_ui::ecs::{
    UiChord, UiClipboard, UiComposition, UiFileDialog, UiNav, UiPointer, UiScreen, UiTextEntry,
};

/// A world running the UI kit, and the scripts loaded into it.
///
/// Fields drop in order: the binding, the scripts, then the world.
pub struct UiHost {
    pub ecs: Ecs,
    /// The kit's scripts, then the application's, in load order.
    scripts: Vec<Script>,
    pub world: World,
    size: [f32; 2],
    /// Text a copy put on the clipboard, until the window publishes it.
    outbox: RefCell<Option<String>>,
    /// The recording being made, and the tick it is at.
    recorder: RefCell<Option<(InputRecording, u64)>>,
}

impl UiHost {
    /// A world with the UI flecs module and the whole widget kit, `size` UI
    /// units big. The application's scripts go into the flecs module
    /// `flecs_module`.
    pub fn new(flecs_module: &str, size: [f32; 2]) -> Result<UiHost, String> {
        let world = World::new();
        let ecs = Ecs::new(&world, 1, flecs_module).map_err(|e| e.0)?;
        sage_ui::setup(&world, &ecs)?;
        world.set(UiScreen {
            w: size[0],
            h: size[1],
        });
        let scripts = sage_ui::load_kit(&ecs)?;
        Ok(UiHost {
            ecs,
            scripts,
            world,
            size,
            outbox: Default::default(),
            recorder: Default::default(),
        })
    }

    /// Loads one of the application's scripts.
    pub fn load(&mut self, name: &str, source: &str) -> Result<(), String> {
        self.load_with(name, source, vec![])
    }

    /// Loads a script with more globals (a host object it talks to).
    pub fn load_with(
        &mut self,
        name: &str,
        source: &str,
        extra: Vec<(String, Py<PyAny>)>,
    ) -> Result<(), String> {
        let script = self.ecs.load_with(name, source, extra).map_err(|e| e.0)?;
        self.scripts.push(script);
        Ok(())
    }

    /// Loads one script written as several files, `(part name, source)` in
    /// order, sharing one namespace.
    pub fn load_parts(
        &mut self,
        name: &str,
        parts: &[(&str, &str)],
        extra: Vec<(String, Py<PyAny>)>,
    ) -> Result<(), String> {
        let script = self.ecs.load_parts(name, parts, extra).map_err(|e| e.0)?;
        self.scripts.push(script);
        Ok(())
    }

    /// The UI's size in UI units.
    pub fn size(&self) -> [f32; 2] {
        self.size
    }

    // ---- Python: the application's names -------------------------------------------

    /// The application's scripts (everything loaded after the kit).
    fn app_scripts(&self) -> &[Script] {
        &self.scripts[sage_ui::KIT.len()..]
    }

    /// The names of every application script, merged: a later script wins
    /// over an earlier one for a name they share.
    fn namespace<'py>(&self, py: Python<'py>) -> Bound<'py, PyDict> {
        let ns = PyDict::new(py);
        for script in self.app_scripts() {
            ns.update(script.globals(py).as_mapping())
                .expect("merge globals");
        }
        ns
    }

    /// Evaluates a Python expression with the application's names.
    pub fn eval<T: for<'py> FromPyObjectOwned<'py>>(&self, expr: &str) -> T {
        Python::attach(|py| {
            let code = CString::new(expr).expect("no NUL in an expression");
            py.eval(&code, Some(&self.namespace(py)), None)
                .unwrap_or_else(|e| panic!("{expr}: {}", sage_script::describe(py, &e)))
                .extract()
                .unwrap_or_else(|_| panic!("{expr}: wrong type"))
        })
    }

    /// Runs Python statements with the application's names. Names they bind
    /// are not kept; change state through the objects the scripts hold.
    pub fn run(&self, code: &str) {
        Python::attach(|py| {
            let code = CString::new(code).expect("no NUL in code");
            py.run(&code, Some(&self.namespace(py)), None)
                .unwrap_or_else(|e| panic!("{}", sage_script::describe(py, &e)));
        });
    }

    /// Calls the function a script defined as `name`, with no arguments (the
    /// latest script that defines it).
    pub fn call(&self, name: &str) {
        Python::attach(|py| {
            let f = self
                .app_scripts()
                .iter()
                .rev()
                .find_map(|s| s.globals(py).get_item(name).ok().flatten())
                .unwrap_or_else(|| panic!("no script defines {name}"));
            if let Err(e) = f.call0() {
                panic!("{name}: {}", sage_script::describe(py, &e));
            }
        });
    }

    /// The computed rectangle of the entity `expr` evaluates to: x, y, w, h.
    pub fn rect(&self, expr: &str) -> [f32; 4] {
        let sid = self.eval::<u64>(&format!("({expr}).sid"));
        let e = self
            .ecs
            .entity_of_sid(sid)
            .unwrap_or_else(|| panic!("{expr} (StableId {sid}) is not in the world"));
        let id = self.ecs.lookup_path("sage.ui.Rect");
        ["x", "y", "w", "h"].map(|f| self.ecs.field(e, id, f).expect("a UI node has a Rect") as f32)
    }

    pub fn center(&self, expr: &str) -> [f32; 2] {
        let [x, y, w, h] = self.rect(expr);
        [x + w / 2.0, y + h / 2.0]
    }

    // ---- a tick: input in, clipboard out ---------------------------------------------

    /// Writes one tick of input into the UI's singletons. While recording,
    /// the input is first rounded the way the file stores it, so a replay is
    /// exact.
    pub fn set_input(&self, i: &UiTickInput) {
        let quantized;
        let i = match self.recorder.borrow_mut().as_mut() {
            Some((rec, tick)) => {
                i.record_into(rec, *tick);
                quantized = UiTickInput {
                    paste: i.paste.clone(),
                    ..UiTickInput::from_recording(rec, *tick)
                };
                *tick += 1;
                &quantized
            }
            None => i,
        };
        self.world.set(UiPointer {
            x: i.pointer.map_or(0.0, |p| p[0]),
            y: i.pointer.map_or(0.0, |p| p[1]),
            present: i.pointer.is_some() as u32,
            down: i.down as u32 | (i.secondary as u32) << 1 | (i.middle as u32) << 2,
            wheel: i.wheel,
        });
        self.world.set(UiNav { bits: i.nav });
        self.world.set(UiChord {
            key: i.chord.map_or(0, |c| c[0]),
            mods: i.chord.map_or(0, |c| c[1]),
        });
        if let Some(text) = &i.paste {
            let id = self.ecs.intern(text);
            self.world.set(UiClipboard { text: id, out: 0 });
        }
        let composing = if i.composing.is_empty() {
            0
        } else {
            self.ecs.intern(&i.composing)
        };
        self.world.set(UiComposition { text: composing });
        self.world.set(UiTextEntry {
            ch: i.ch.map_or(0, |c| c as u32),
        });
    }

    /// Ends a tick: a script error stops the application (its scripts are its
    /// own code), and text the UI copied waits for the window.
    pub fn finish_tick(&self) {
        if let Some(fault) = self.ecs.fault() {
            panic!("script error: {fault}");
        }
        let clip = self.world.get::<&UiClipboard>(|c| *c);
        if clip.out != 0 {
            *self.outbox.borrow_mut() = self.string(clip.text);
            self.world.set(UiClipboard { out: 0, ..clip });
        }
    }

    fn string(&self, id: u64) -> Option<String> {
        self.ecs.strings().borrow().get(id).map(str::to_string)
    }

    /// The text on the UI's clipboard (what a paste would insert).
    pub fn clipboard(&self) -> String {
        let id = self.world.get::<&UiClipboard>(|c| c.text);
        self.string(id).unwrap_or_default()
    }

    /// Starts recording every tick of input (`sage-input v2`).
    pub fn start_recording(&self, seed: u64) {
        *self.recorder.borrow_mut() = Some((InputRecording::new(seed), 0));
    }

    /// Stops recording and returns what was recorded.
    pub fn take_recording(&self) -> Option<InputRecording> {
        self.recorder.borrow_mut().take().map(|(r, _)| r)
    }

    /// The hash of everything the binding knows about the UI world.
    pub fn state_hash(&self) -> u64 {
        let mut h = StateHasher::new();
        self.ecs.hash_into(&mut h);
        h.finish()
    }

    // ---- what is on screen -----------------------------------------------------------

    pub fn frame(&self) -> UiFrame {
        sage_ui::frame(&self.world, &self.ecs)
    }

    /// Everything the UI shows as text, in draw order (a wrapped line's
    /// breaks become spaces).
    pub fn texts(&self) -> Vec<String> {
        self.frame()
            .draws
            .into_iter()
            .filter_map(|d| match d {
                UiDraw::Text { text, .. } => Some(text.replace('\n', " ")),
                _ => None,
            })
            .collect()
    }

    /// What a screen reader would be told about the UI now, as indented text.
    pub fn access_outline(&self) -> String {
        sage_ui::access_outline(&self.world, &self.ecs)
    }

    // ---- what a window asks of the UI --------------------------------------------------

    /// Text a copy or cut put on the clipboard since the last call.
    pub fn take_clipboard(&self) -> Option<String> {
        self.outbox.borrow_mut().take()
    }

    /// A file dialog the UI wants shown, if any (asked once).
    pub fn take_file_request(&self) -> Option<FileRequest> {
        let d = self.world.get::<&UiFileDialog>(|d| *d);
        if d.request == 0 {
            return None;
        }
        self.world.set(UiFileDialog { request: 0, ..d });
        Some(FileRequest {
            save: d.request == 2,
        })
    }

    /// The answer to a file dialog: the chosen path, or `None` if cancelled.
    pub fn file_chosen(&self, path: Option<String>) {
        sage_ui::answer_file_dialog(&self.world, &self.ecs, path.as_deref());
    }

    /// The accessibility tree, for a platform adapter.
    pub fn access_tree(&self) -> accesskit::TreeUpdate {
        sage_ui::access_update(&self.world, &self.ecs)
    }

    /// An action assistive technology asked for (focus, click).
    pub fn access_action(&self, request: accesskit::ActionRequest) {
        sage_ui::perform_access_action(&self.world, request.target_node.0, request.action);
    }
}

/// An application built around a [`UiHost`].
///
/// Implement [`UiHosted::ui`] (and whatever of the rest the application has)
/// and it can be driven tick by tick, by gestures, by a recording or by a
/// window.
pub trait UiHosted {
    fn ui(&self) -> &UiHost;

    /// The application's own part of a tick, after the UI world progressed:
    /// stepping an embedded simulation, rebuilding views, syncing audio.
    fn after_ui(&self) {}

    /// Idle ticks that end every gesture. An application whose views are
    /// rebuilt after the tick that handled an event needs one for the new
    /// widgets to be laid out before the next gesture aims at them.
    fn settle(&self) -> u32 {
        0
    }

    /// The text face at `scale` device pixels per UI unit, when it is not the
    /// built-in one.
    fn font(&self, _scale: u32) -> Option<BitmapFont> {
        None
    }

    /// Image assets the UI draws, by `AssetId`.
    fn images(&self) -> Vec<(u64, Image)> {
        vec![]
    }

    /// The color behind the UI where it does not cover the window.
    fn clear_color(&self) -> [u8; 4] {
        [0, 0, 0, 255]
    }

    /// Interleaved stereo samples for the audio device, if the application
    /// makes sound.
    fn mix_audio(&self, _frames: usize) -> Option<Vec<f32>> {
        None
    }

    // ---- driving the UI the way a window does ----------------------------------------

    /// One tick without new input.
    fn step(&self) {
        self.ui().world.progress();
        self.after_ui();
        self.ui().finish_tick();
    }

    /// One tick with the given input.
    fn input(&self, i: &UiTickInput) {
        self.ui().set_input(i);
        self.step();
    }

    fn idle(&self, n: u32) {
        for _ in 0..n {
            self.input(&UiTickInput::default());
        }
    }

    /// Moves the pointer to `p` without pressing.
    fn hover(&self, p: [f32; 2]) {
        self.input(&UiTickInput {
            pointer: Some(p),
            ..UiTickInput::default()
        });
    }

    /// Presses and releases the left button at `p`.
    fn click_at(&self, p: [f32; 2]) {
        for down in [true, false] {
            self.input(&UiTickInput {
                pointer: Some(p),
                down,
                ..UiTickInput::default()
            });
        }
        self.idle(self.settle());
    }

    /// A right click: the secondary button down and up at `p`.
    fn right_click_at(&self, p: [f32; 2]) {
        for secondary in [true, false] {
            self.input(&UiTickInput {
                pointer: Some(p),
                secondary,
                ..UiTickInput::default()
            });
        }
        self.idle(self.settle());
    }

    /// Presses and releases the middle button at `p`.
    fn middle_click_at(&self, p: [f32; 2]) {
        for middle in [true, false] {
            self.input(&UiTickInput {
                pointer: Some(p),
                middle,
                ..UiTickInput::default()
            });
        }
        self.idle(self.settle());
    }

    /// Presses at `from`, moves to `to` in a few steps, and releases there.
    fn drag(&self, from: [f32; 2], to: [f32; 2]) {
        let at = |t: f32| {
            [
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
            ]
        };
        for (pos, down) in [
            (at(0.0), true),
            (at(0.5), true),
            (at(1.0), true),
            (at(1.0), false),
        ] {
            self.input(&UiTickInput {
                pointer: Some(pos),
                down,
                ..UiTickInput::default()
            });
        }
        self.idle(1 + self.settle());
    }

    fn wheel_at(&self, p: [f32; 2], notches: f32) {
        self.input(&UiTickInput {
            pointer: Some(p),
            wheel: notches,
            ..UiTickInput::default()
        });
    }

    /// Types `text`, one character per tick, with an idle tick between.
    fn type_text(&self, text: &str) {
        for c in text.chars() {
            self.input(&UiTickInput {
                ch: Some(c),
                ..UiTickInput::default()
            });
            self.input(&UiTickInput::default());
        }
        self.idle(self.settle());
    }

    /// Presses and releases the navigation bits `bits`.
    fn press(&self, bits: u32) {
        self.input(&UiTickInput {
            nav: bits,
            ..UiTickInput::default()
        });
        self.idle(1 + self.settle());
    }

    /// One key chord (`key` as `ui.parse_chord` gives it), then a tick
    /// without it.
    fn chord(&self, key: u32, mods: u32) {
        self.input(&UiTickInput {
            chord: Some([key, mods]),
            ..UiTickInput::default()
        });
        self.idle(1 + self.settle());
    }

    /// Clicks the center of the entity `expr` evaluates to.
    fn click(&self, expr: &str) {
        self.click_at(self.ui().center(expr));
    }

    /// Plays a recording from the start, one tick per recorded tick. Returns
    /// the UI world's hash after each.
    fn replay(&self, rec: &InputRecording) -> Vec<u64> {
        (0..rec.len)
            .map(|t| {
                self.input(&UiTickInput::from_recording(rec, t));
                self.ui().state_hash()
            })
            .collect()
    }

    /// Renders the UI into an image of `size` pixels with the CPU reference
    /// rasterizer (the same batch the GPU draws).
    fn render(&self, size: [u32; 2]) -> Image {
        let scale = (size[0] as f32 / self.ui().size()[0]).round().max(1.0) as u32;
        let font = self.font(scale).unwrap_or_else(BitmapFont::builtin);
        sage_platform::sprite::render_ui(
            &self.ui().frame(),
            &font,
            &self.images(),
            self.clear_color(),
            size,
        )
    }
}

/// The window loop's view of a hosted application.
struct Windowed<'a, A: UiHosted>(&'a mut A);

impl<A: UiHosted> UiApp for Windowed<'_, A> {
    fn size(&self) -> [f32; 2] {
        self.0.ui().size()
    }

    fn tick(&mut self, input: &UiTickInput) {
        self.0.input(input);
    }

    fn frame(&self) -> UiFrame {
        self.0.ui().frame()
    }

    fn images(&self) -> Vec<(u64, Image)> {
        self.0.images()
    }

    fn font(&self, scale: u32) -> Option<BitmapFont> {
        self.0.font(scale)
    }

    fn mix_audio(&mut self, frames: usize) -> Option<Vec<f32>> {
        self.0.mix_audio(frames)
    }

    fn take_clipboard(&mut self) -> Option<String> {
        self.0.ui().take_clipboard()
    }

    fn take_file_request(&mut self) -> Option<FileRequest> {
        self.0.ui().take_file_request()
    }

    fn file_chosen(&mut self, path: Option<String>) {
        self.0.ui().file_chosen(path);
    }

    fn access_tree(&self) -> accesskit::TreeUpdate {
        self.0.ui().access_tree()
    }

    fn access_action(&mut self, request: accesskit::ActionRequest) {
        self.0.ui().access_action(request);
    }
}

/// Shows a hosted application in a window until it is closed (GPU only).
pub fn run_ui_windowed<A: UiHosted>(app: &mut A, options: &WindowOptions) -> Result<UiRun, String> {
    sage_platform::ui_window::run_ui_windowed(&mut Windowed(app), options)
}
