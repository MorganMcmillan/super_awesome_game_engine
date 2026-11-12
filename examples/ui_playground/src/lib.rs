//! A playground for the UI module: every widget, themed by style prefabs.
//!
//! `scripts/theme.py` is the palette and the prefabs, `scripts/playground.py`
//! is the screen and its behavior. This crate wires them to a world, feeds
//! the UI's input singletons from a window (or from a script of inputs, for
//! tests and screenshots), and turns the UI into pixels.

use flecs_ecs::prelude::*;
use pyo3::prelude::*;
use sage_assets::{AssetId, BitmapFont, Image};
use sage_core::host::{NAV_ACCEPT, NAV_NEXT, UiFrame};
use sage_ecs_py::Ecs;
use sage_platform::sprite::{UiResources, compose_ui, rasterize};
use sage_platform::ui_window::{UiApp, UiTickInput};
use sage_script::Script;
use sage_ui::ecs::{
    UiChord, UiClipboard, UiComposition, UiFileDialog, UiNav, UiPointer, UiScreen, UiTextEntry,
};

pub const THEME: &str = include_str!("../scripts/theme.py");
pub const SCRIPT: &str = include_str!("../scripts/playground.py");

/// The pages loaded after the main script, by name. A page registers
/// itself with `world.add_page`.
pub const PAGES: &[(&str, &str)] = &[
    ("style", include_str!("../scripts/pages/style.py")),
    ("popups", include_str!("../scripts/pages/popups.py")),
    ("text", include_str!("../scripts/pages/text.py")),
    ("layout", include_str!("../scripts/pages/layout.py")),
    ("panels", include_str!("../scripts/pages/panels.py")),
    ("drag", include_str!("../scripts/pages/drag.py")),
    ("canvas", include_str!("../scripts/pages/canvas.py")),
    ("access", include_str!("../scripts/pages/access.py")),
    ("pickers", include_str!("../scripts/pages/pickers.py")),
    ("media", include_str!("../scripts/pages/media.py")),
];

pub mod assets;

/// A toy simulation the CANVAS page embeds in a `Viewport`: a ball that
/// bounces around a court and can be dragged with the mouse. It belongs to
/// the host, not to the UI world: it is not hashed or saved with it.
#[derive(Clone, Debug, PartialEq)]
pub struct Bouncer {
    pub tick: u64,
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    held_at: Option<[f32; 2]>,
}

/// The id the CANVAS page's viewport takes its frame from.
pub const BOUNCER_VIEW: u32 = 1;

impl Default for Bouncer {
    fn default() -> Self {
        Bouncer {
            tick: 0,
            pos: [80.0, 45.0],
            vel: [0.9, 0.6],
            held_at: None,
        }
    }
}

impl Bouncer {
    pub const WORLD: [f32; 2] = [160.0, 90.0];
    pub const RADIUS: f32 = 3.0;

    /// One tick. `pointer` is `(x, y, over, buttons)` from the viewport: the
    /// ball follows the primary button, and flies on with the pointer's
    /// last movement when it is let go.
    pub fn step(&mut self, pointer: Option<(f32, f32, bool, u32)>) {
        self.tick += 1;
        let held = pointer.filter(|p| p.3 & 1 != 0);
        match (held, self.held_at) {
            (Some((x, y, _, _)), last) => {
                if let Some(last) = last {
                    self.vel = [(x - last[0]) * 0.5, (y - last[1]) * 0.5];
                }
                self.pos = [x, y];
                self.held_at = Some([x, y]);
            }
            (None, _) => {
                self.held_at = None;
                self.pos[0] += self.vel[0];
                self.pos[1] += self.vel[1];
            }
        }
        for axis in 0..2 {
            let (lo, hi) = (Self::RADIUS, Self::WORLD[axis] - Self::RADIUS);
            if self.pos[axis] < lo || self.pos[axis] > hi {
                self.pos[axis] = self.pos[axis].clamp(lo, hi);
                if held.is_none() {
                    self.vel[axis] = -self.vel[axis];
                }
            }
        }
    }

    pub fn frame(&self) -> sage_core::host::Frame {
        use sage_core::StableId;
        use sage_core::host::{DrawKind, DrawRect, Frame};
        let r = |id, kind, center, half| DrawRect {
            id: StableId(id),
            kind,
            center,
            half,
        };
        Frame {
            tick: self.tick,
            world_size: Self::WORLD,
            rects: vec![
                r(1, DrawKind::Court, [80.0, 45.0], [80.0, 45.0]),
                r(2, DrawKind::Wall, [80.0, 1.0], [80.0, 1.0]),
                r(3, DrawKind::Wall, [80.0, 89.0], [80.0, 1.0]),
                r(4, DrawKind::Paddle, [6.0, 45.0], [1.5, 9.0]),
                r(5, DrawKind::Paddle, [154.0, 45.0], [1.5, 9.0]),
                r(9, DrawKind::Ball, self.pos, [Self::RADIUS, Self::RADIUS]),
            ],
            hud: vec![format!("TICK {}", self.tick)],
        }
    }
}

/// The UI's size in UI units. A window shows it at a whole-number scale:
/// 960x600 pixels at 2, 1440x900 at 3.
pub const SCREEN: [f32; 2] = [480.0, 300.0];

/// The clear color behind the UI (the app's own background covers it).
pub const CLEAR: [u8; 4] = [15, 13, 11, 255];

/// Fields drop in order: the binding, the scripts, then the world.
pub struct Playground {
    pub ecs: Ecs,
    _kit: Vec<Script>,
    _theme: Script,
    script: Script,
    pages: Vec<Script>,
    pub world: World,
    /// A TrueType face to draw text with instead of the built-in one (the
    /// bytes; rasterized for the scale it is drawn at).
    face: std::cell::RefCell<Option<(Vec<u8>, bool)>>,
    /// Audio for the MEDIA page: sounds by asset id, one voice per player, and
    /// the mixer they play on. Presentation: none of it is world state.
    sounds: std::collections::BTreeMap<u64, std::rc::Rc<sage_assets::Sound>>,
    voices: std::cell::RefCell<std::collections::BTreeMap<u64, sage_platform::media::MediaVoice>>,
    mixer: std::cell::RefCell<sage_platform::mixer::Mixer>,
    /// The simulation embedded in the CANVAS page's viewport.
    pub bouncer: std::cell::RefCell<Bouncer>,
    /// The recording being made, and the tick it is at (`start_recording`).
    recorder: std::cell::RefCell<Option<(sage_core::input::InputRecording, u64)>>,
    /// Text a copy put on the clipboard, until the window publishes it.
    outbox: std::cell::RefCell<Option<String>>,
}

impl Playground {
    pub fn new() -> Result<Playground, String> {
        // No proportional face until one is asked for (the metrics are per
        // thread, and a test thread may have had one).
        sage_core::host::set_text_metrics(None);
        let world = World::new();
        let ecs = Ecs::new(&world, 1, "playground").map_err(|e| e.0)?;
        sage_ui::setup(&world, &ecs)?;
        world.set(UiScreen {
            w: SCREEN[0],
            h: SCREEN[1],
        });
        let kit = sage_ui::load_kit(&ecs)?;
        let theme = ecs.load("theme", THEME).map_err(|e| e.0)?;
        for (_, image) in assets::all().iter().map(|(p, id, i)| (p, (id, i))) {
            let (id, i) = image;
            sage_ui::register_image(&ecs, *id, [i.width, i.height]);
        }
        let script = ecs.load("playground", SCRIPT).map_err(|e| e.0)?;
        let mut pages = vec![];
        for (name, source) in PAGES {
            pages.push(ecs.load(name, source).map_err(|e| e.0)?);
        }
        let p = Playground {
            ecs,
            _kit: kit,
            _theme: theme,
            script,
            pages,
            world,
            face: Default::default(),
            sounds: [(
                AssetId::of_path(assets::TONE).0,
                std::rc::Rc::new(assets::tone()),
            )]
            .into(),
            voices: Default::default(),
            mixer: std::cell::RefCell::new(sage_platform::mixer::Mixer::new(
                sage_platform::audio::OUT_RATE,
            )),
            bouncer: Default::default(),
            recorder: Default::default(),
            outbox: Default::default(),
        };
        sage_ui::set_viewport_frame(&p.ecs, BOUNCER_VIEW, p.bouncer.borrow().frame());
        p.step();
        Ok(p)
    }

    fn step(&self) {
        self.world.progress();
        // The embedded simulation: forward the pointer the viewport reports,
        // step it, and give the viewport what it drew.
        let mut pointer = None;
        self.world
            .new_query::<&sage_ui::ecs::UiViewport>()
            .each(|v| {
                if v.source == BOUNCER_VIEW {
                    pointer = Some((v.x, v.y, v.over != 0, v.down));
                }
            });
        let frame = {
            let mut b = self.bouncer.borrow_mut();
            b.step(pointer);
            b.frame()
        };
        sage_ui::set_viewport_frame(&self.ecs, BOUNCER_VIEW, frame);
        self.sync_media();
        if let Some(fault) = self.ecs.fault() {
            panic!("script error: {fault}");
        }
    }

    /// The names of the main script and every page script, merged: later
    /// pages win over earlier ones for a name they share.
    fn namespace<'py>(&self, py: Python<'py>) -> Bound<'py, pyo3::types::PyDict> {
        let ns = self.script.globals(py).copy().expect("copy globals");
        for page in &self.pages {
            ns.update(page.globals(py).as_mapping())
                .expect("merge globals");
        }
        ns
    }

    /// Evaluates a Python expression with the names of every script.
    pub fn eval<T: for<'py> FromPyObjectOwned<'py>>(&self, expr: &str) -> T {
        Python::attach(|py| {
            let code = std::ffi::CString::new(expr).unwrap();
            let ns = self.namespace(py);
            py.eval(&code, Some(&ns), None)
                .unwrap_or_else(|e| panic!("{expr}: {}", sage_script::describe(py, &e)))
                .extract()
                .unwrap_or_else(|_| panic!("{expr}: wrong type"))
        })
    }

    /// Runs Python statements with the names of every script.
    pub fn run(&self, code: &str) {
        Python::attach(|py| {
            let code = std::ffi::CString::new(code).unwrap();
            let ns = self.namespace(py);
            py.run(&code, Some(&ns), None)
                .unwrap_or_else(|e| panic!("{}", sage_script::describe(py, &e)));
        });
    }

    /// The entity held in the script variable `var` (`primary`, `volume`...).
    fn entity(&self, var: &str) -> u64 {
        let sid = self.eval::<u64>(&format!("{var}.sid"));
        self.ecs
            .entity_of_sid(sid)
            .unwrap_or_else(|| panic!("{var} (StableId {sid}) is not in the world"))
    }

    /// The computed rectangle of the widget in `var`: x, y, width, height.
    pub fn rect(&self, var: &str) -> [f32; 4] {
        let e = self.entity(var);
        let id = self.ecs.lookup_path("sage.ui.Rect");
        ["x", "y", "w", "h"].map(|f| self.ecs.field(e, id, f).unwrap() as f32)
    }

    pub fn center(&self, var: &str) -> [f32; 2] {
        let [x, y, w, h] = self.rect(var);
        [x + w / 2.0, y + h / 2.0]
    }

    /// Starts recording every tick of input (`sage-input v2`). The input is
    /// rounded the way the file stores it, so a replay is exact.
    pub fn start_recording(&self, seed: u64) {
        *self.recorder.borrow_mut() = Some((sage_core::input::InputRecording::new(seed), 0));
    }

    /// Stops recording and returns what was recorded.
    pub fn take_recording(&self) -> Option<sage_core::input::InputRecording> {
        self.recorder.borrow_mut().take().map(|(r, _)| r)
    }

    /// Plays a recording from the start: one tick per recorded tick. Returns
    /// the world's hash after each tick.
    pub fn replay(&self, rec: &sage_core::input::InputRecording) -> Vec<u64> {
        (0..rec.len)
            .map(|t| {
                self.input(&UiTickInput::from_recording(rec, t));
                let mut h = sage_core::StateHasher::new();
                self.ecs.hash_into(&mut h);
                h.finish()
            })
            .collect()
    }

    /// One tick with the given input, the way the window loop does it.
    pub fn input(&self, i: &UiTickInput) {
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
        self.step();
        let clip = self.world.get::<&UiClipboard>(|c| *c);
        if clip.out != 0 {
            let text = self
                .ecs
                .strings()
                .borrow()
                .get(clip.text)
                .map(str::to_string);
            *self.outbox.borrow_mut() = text;
            self.world.set(UiClipboard { out: 0, ..clip });
        }
    }

    /// The text on the UI's clipboard (what a paste would insert).
    pub fn clipboard(&self) -> String {
        let id = self.world.get::<&UiClipboard>(|c| c.text);
        self.ecs
            .strings()
            .borrow()
            .get(id)
            .unwrap_or("")
            .to_string()
    }

    pub fn hover(&self, p: [f32; 2]) {
        self.input(&UiTickInput {
            pointer: Some(p),
            ..UiTickInput::default()
        });
    }

    /// Presses and releases the left button at `p`.
    pub fn click_at(&self, p: [f32; 2]) {
        for down in [true, false] {
            self.input(&UiTickInput {
                pointer: Some(p),
                down,
                ..UiTickInput::default()
            });
        }
    }

    /// Presses and releases the middle button at `p`.
    pub fn middle_click_at(&self, p: [f32; 2]) {
        for middle in [true, false] {
            self.input(&UiTickInput {
                pointer: Some(p),
                middle,
                ..UiTickInput::default()
            });
        }
    }

    /// One key chord (`key` as `ui.parse_chord` gives it), then an idle tick.
    pub fn chord(&self, key: u32, mods: u32) {
        self.input(&UiTickInput {
            chord: Some([key, mods]),
            ..UiTickInput::default()
        });
        self.input(&UiTickInput::default());
    }

    /// Presses at `from`, moves to `to` in a few steps, and releases there.
    pub fn drag(&self, from: [f32; 2], to: [f32; 2]) {
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
        self.idle(1);
    }

    /// Clicks the center of the widget in `var`.
    pub fn click(&self, var: &str) {
        self.click_at(self.center(var));
    }

    pub fn wheel_at(&self, p: [f32; 2], notches: f32) {
        self.input(&UiTickInput {
            pointer: Some(p),
            wheel: notches,
            ..UiTickInput::default()
        });
    }

    /// Types `text`, one character per tick, with an idle tick between.
    pub fn type_text(&self, text: &str) {
        for c in text.chars() {
            self.input(&UiTickInput {
                ch: Some(c),
                ..UiTickInput::default()
            });
            self.input(&UiTickInput::default());
        }
    }

    /// Presses and releases the navigation bits `bits`.
    pub fn press(&self, bits: u32) {
        self.input(&UiTickInput {
            nav: bits,
            ..UiTickInput::default()
        });
        self.input(&UiTickInput::default());
    }

    pub fn tab(&self) {
        self.press(NAV_NEXT);
    }

    pub fn accept(&self) {
        self.press(NAV_ACCEPT);
    }

    /// Runs `n` idle ticks (the progress bar fills itself).
    pub fn idle(&self, n: u32) {
        for _ in 0..n {
            self.input(&UiTickInput::default());
        }
    }

    /// Clicks the tab of the page called `name` and lets the page appear.
    pub fn show_page(&self, name: &str) {
        let i = self.eval::<usize>(&format!("page_names.index({name:?})"));
        self.click(&format!("tab_buttons[{i}]"));
        self.idle(3);
    }

    /// The text of a label entity held in `var`.
    pub fn text(&self, var: &str) -> String {
        self.eval(&format!("text_of({var})"))
    }

    /// Draws text with this TrueType face (a `.ttf` file's bytes) instead of
    /// the built-in bitmap face.
    pub fn set_face(&self, ttf: Vec<u8>) -> Result<(), String> {
        BitmapFont::from_ttf(&ttf, 1)?;
        sage_core::host::set_text_metrics(None);
        *self.face.borrow_mut() = Some((ttf, false));
        Ok(())
    }

    /// Draws text with a proportional, shaped face: static text (labels,
    /// buttons, paragraphs) is measured and wrapped with the face's own
    /// advances and kerning; text inputs stay on the grid.
    pub fn set_face_proportional(&self, ttf: Vec<u8>) -> Result<(), String> {
        let font = BitmapFont::from_ttf_proportional(&ttf, 1)?;
        sage_core::host::set_text_metrics(font.shaped.clone().map(|s| s as _));
        *self.face.borrow_mut() = Some((ttf, true));
        Ok(())
    }

    /// The text face in use at `scale` device pixels per UI unit.
    fn face_at(&self, scale: u32) -> Option<BitmapFont> {
        let face = self.face.borrow();
        let (bytes, proportional) = face.as_ref()?;
        if *proportional {
            BitmapFont::from_ttf_proportional(bytes, scale).ok()
        } else {
            BitmapFont::from_ttf(bytes, scale).ok()
        }
    }

    /// Keeps a mixer voice in step with every media player the UI has: the
    /// UI owns what is playing, this only tells the mixer.
    fn sync_media(&self) {
        let mut players = vec![];
        self.world
            .new_query::<(&sage_ecs_py::StableIdC, &sage_ui::ecs::UiMedia)>()
            .each(|(sid, m)| players.push((sid.value, *m)));
        let mut mixer = self.mixer.borrow_mut();
        let mut voices = self.voices.borrow_mut();
        for (sid, m) in players {
            let Some(sound) = self.sounds.get(&m.source) else {
                continue;
            };
            voices.entry(sid).or_default().sync(
                &mut mixer,
                sound,
                sage_platform::media::MediaState {
                    playing: m.playing != 0,
                    position: m.position,
                    length: m.length,
                    volume: m.volume,
                    looping: m.looping != 0,
                },
            );
        }
    }

    /// Mixes `frames` stereo frames of what the players are playing (what the
    /// window sends to the audio device).
    pub fn audio(&self, frames: usize) -> Vec<f32> {
        self.mixer.borrow_mut().mix(frames)
    }

    /// How many voices are on the mixer.
    pub fn voice_count(&self) -> usize {
        self.mixer.borrow().voice_count()
    }

    /// What a screen reader would be told about the UI now, as indented text.
    pub fn access_outline(&self) -> String {
        sage_ui::access_outline(&self.world, &self.ecs)
    }

    pub fn ui_frame(&self) -> UiFrame {
        sage_ui::frame(&self.world, &self.ecs)
    }

    /// Renders the current UI into an image of `size` pixels with the CPU
    /// reference rasterizer (the same batch the GPU draws).
    pub fn render(&self, size: [u32; 2]) -> Image {
        let scale = (size[0] as f32 / SCREEN[0]).round().max(1.0) as u32;
        let font = self.face_at(scale).unwrap_or_else(BitmapFont::builtin);
        let mut res = UiResources::new(2);
        for (_, id, image) in assets::all() {
            res.register_asset(id, image);
        }
        let batch = compose_ui(
            &self.ui_frame(),
            [size[0] as f32, size[1] as f32],
            &font,
            &mut res,
        );
        let textures = res.textures(&font);
        let px = rasterize(&batch, &textures, CLEAR, size);
        let mut img = Image::solid(size[0], size[1], [0, 0, 0, 255]);
        for (i, p) in px.iter().enumerate() {
            img.set_pixel(i as u32 % size[0], i as u32 / size[0], *p);
        }
        img
    }
}

impl UiApp for Playground {
    fn size(&self) -> [f32; 2] {
        SCREEN
    }

    fn tick(&mut self, input: &UiTickInput) {
        self.input(input);
    }

    fn frame(&self) -> UiFrame {
        self.ui_frame()
    }

    fn take_clipboard(&mut self) -> Option<String> {
        self.outbox.borrow_mut().take()
    }

    fn access_tree(&self) -> accesskit::TreeUpdate {
        sage_ui::access_update(&self.world, &self.ecs)
    }

    fn access_action(&mut self, request: accesskit::ActionRequest) {
        sage_ui::perform_access_action(&self.world, request.target_node.0, request.action);
    }

    fn take_file_request(&mut self) -> Option<sage_platform::ui_window::FileRequest> {
        let d = self.world.get::<&UiFileDialog>(|d| *d);
        if d.request == 0 {
            return None;
        }
        self.world.set(UiFileDialog { request: 0, ..d });
        Some(sage_platform::ui_window::FileRequest {
            save: d.request == 2,
        })
    }

    fn file_chosen(&mut self, path: Option<String>) {
        sage_ui::answer_file_dialog(&self.world, &self.ecs, path.as_deref());
    }

    fn font(&self, scale: u32) -> Option<BitmapFont> {
        self.face_at(scale)
    }

    fn mix_audio(&mut self, frames: usize) -> Option<Vec<f32>> {
        Some(self.audio(frames))
    }

    fn images(&self) -> Vec<(u64, Image)> {
        assets::all()
            .into_iter()
            .map(|(_, id, i)| (id, i))
            .collect()
    }
}
