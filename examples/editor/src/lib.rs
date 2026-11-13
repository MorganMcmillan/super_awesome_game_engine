//! The editor (design doc 10.6; milestones M3 and M4).
//!
//! The screen is UI entities written in Python (`scripts/editor.py`) over the
//! engine's own widget kit, as design doc 10.6 decides. This crate wires it:
//! the document and the project it edits live in `sage_editor`, the `editor`
//! object in `bridge.rs` is what scripts call, and the host here feeds the UI
//! its input, runs play in editor, shows the scene in a viewport and turns
//! the UI into pixels.

pub mod bridge;

use bridge::{EditorHost, Shared};
use flecs_ecs::prelude::*;
use pyo3::prelude::*;
use sage_assets::{BitmapFont, Image};
use sage_core::StableId;
use sage_core::host::{DrawKind, DrawRect, Frame, UiFrame};
use sage_ecs_py::Ecs;
use sage_editor::{Mode, Prepare, Project};
use sage_platform::sprite::{UiResources, compose_ui, rasterize};
use sage_platform::ui_window::{UiApp, UiTickInput};
use sage_scene::{Scene, Value};
use sage_script::Script;
use sage_ui::ecs::{
    UiChord, UiClipboard, UiComposition, UiFileDialog, UiNav, UiPointer, UiScreen, UiTextEntry,
    UiViewport,
};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

pub const THEME: &str = include_str!("../scripts/theme.py");
pub const SCRIPT: &str = include_str!("../scripts/editor.py");
/// The game the editor opens when given no project.
pub const SAMPLE_GAME: &str = include_str!("../project/game.py");

/// The UI's size in UI units: 1280x800 pixels at scale 2.
pub const SCREEN: [f32; 2] = [640.0, 400.0];
pub const CLEAR: [u8; 4] = [15, 13, 11, 255];
/// The id the scene viewport takes its frame from.
pub const SCENE_VIEW: u32 = 1;

/// How a game world is prepared before its scripts load. Pong-style hosts
/// register native components here; the sample needs nothing.
pub fn no_prepare() -> Prepare {
    Box::new(|_, _| Ok(()))
}

/// Opens the sample project.
pub fn sample_project() -> Result<Project, String> {
    Project::open(
        "sample",
        vec![("game".into(), SAMPLE_GAME.into())],
        None,
        no_prepare(),
    )
}

/// Opens a project directory: every `*.py` file in name order, and
/// `scene.sage` if it exists.
pub fn open_dir(dir: &std::path::Path) -> Result<Project, String> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .collect();
    names.sort();
    let mut sources = vec![];
    for p in names {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        sources.push((
            name,
            std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?,
        ));
    }
    if sources.is_empty() {
        return Err(format!("{} has no .py files", dir.display()));
    }
    let scene = std::fs::read_to_string(dir.join("scene.sage")).ok();
    let settings = std::fs::read_to_string(dir.join("settings.sage")).ok();
    let name = dir
        .file_name()
        .map_or("project".into(), |n| n.to_string_lossy().to_string());
    let mut project = Project::open(&name, sources, scene.as_deref(), no_prepare())?;
    if let Some(text) = settings {
        project.load_settings(&text);
    }
    Ok(project)
}

/// Fields drop in order: the binding, the scripts, then the world.
pub struct Editor {
    pub ecs: Ecs,
    _kit: Vec<Script>,
    _theme: Script,
    script: Script,
    pub world: World,
    pub shared: Rc<Shared>,
    host_id: u64,
    dir: Option<PathBuf>,
    drag: RefCell<Option<Drag>>,
    was_down: Cell<bool>,
    outbox: RefCell<Option<String>>,
}

#[derive(Clone, Debug)]
struct Drag {
    sid: u64,
    component: String,
    grab: [f32; 2],
}

/// The position component of an entity: the one named Position, else the
/// first with float `x` and `y` fields.
fn position_of(e: &sage_scene::SceneEntity) -> Option<(&sage_scene::SceneComponent, [f32; 2])> {
    let num = |c: &sage_scene::SceneComponent, n: &str| {
        c.fields
            .iter()
            .find(|(f, _)| f == n)
            .and_then(|(_, v)| match v {
                Value::F32(x) => Some(*x),
                Value::F64(x) => Some(*x as f32),
                _ => None,
            })
    };
    let mut best = None;
    for c in &e.components {
        if let (Some(x), Some(y)) = (num(c, "x"), num(c, "y")) {
            if c.path.ends_with("Position") {
                return Some((c, [x, y]));
            }
            best.get_or_insert((c, [x, y]));
        }
    }
    best
}

/// Half extents from a component with `hx` and `hy` (or `half_x`...), else a
/// small default.
fn half_of(e: &sage_scene::SceneEntity) -> [f32; 2] {
    for c in &e.components {
        for (a, b) in [("hx", "hy"), ("half_x", "half_y")] {
            let get = |n: &str| {
                c.fields
                    .iter()
                    .find(|(f, _)| f == n)
                    .and_then(|(_, v)| match v {
                        Value::F32(x) => Some(*x),
                        Value::F64(x) => Some(*x as f32),
                        _ => None,
                    })
            };
            if let (Some(x), Some(y)) = (get(a), get(b)) {
                return [x.abs().max(0.5), y.abs().max(0.5)];
            }
        }
    }
    [3.0, 3.0]
}

/// What the scene viewport shows: every entity with a position, as a box.
pub fn scene_frame(scene: &Scene, selected: Option<u64>, tick: u64) -> Frame {
    let mut rects = vec![];
    let mut size = [160.0f32, 90.0f32];
    for e in &scene.entities {
        let Some((_, p)) = position_of(e) else {
            continue;
        };
        let half = half_of(e);
        size[0] = size[0].max(p[0] + half[0] + 10.0);
        size[1] = size[1].max(p[1] + half[1] + 10.0);
        rects.push(DrawRect {
            id: StableId(e.sid),
            kind: if Some(e.sid) == selected {
                DrawKind::Paddle
            } else {
                DrawKind::Wall
            },
            center: p,
            half,
        });
    }
    let mut all = vec![DrawRect {
        id: StableId(0),
        kind: DrawKind::Court,
        center: [size[0] / 2.0, size[1] / 2.0],
        half: [size[0] / 2.0, size[1] / 2.0],
    }];
    all.extend(rects);
    Frame {
        tick,
        world_size: size,
        rects: all,
        hud: vec![],
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        bridge::unregister(self.host_id);
    }
}

impl Editor {
    pub fn new(project: Project, dir: Option<PathBuf>) -> Result<Editor, String> {
        sage_core::host::set_text_metrics(None);
        sage_editor::blocks::install()?;
        let world = World::new();
        let ecs = Ecs::new(&world, 1, "editor").map_err(|e| e.0)?;
        sage_ui::setup(&world, &ecs)?;
        world.set(UiScreen {
            w: SCREEN[0],
            h: SCREEN[1],
        });
        let kit = sage_ui::load_kit(&ecs)?;
        let theme = ecs.load("theme", THEME).map_err(|e| e.0)?;
        let mut project = project;
        // Blocks glow while the game runs: scripts report their lines.
        project.set_instrument(true)?;
        let shared = Rc::new(Shared {
            project: RefCell::new(project),
            requests: Default::default(),
            extra: Cell::new(1),
            log: Default::default(),
            paused: Cell::new(false),
        });
        let host_obj = EditorHost::register(shared.clone());
        let host_id = host_obj.id();
        let host = Python::attach(|py| Py::new(py, host_obj).map(|h| h.into_any()))
            .map_err(|e| Python::attach(|py| sage_script::describe(py, &e).0))?;
        let script = ecs
            .load_with("editor", SCRIPT, vec![("editor".to_string(), host)])
            .map_err(|e| e.0)?;
        let ed = Editor {
            ecs,
            _kit: kit,
            _theme: theme,
            script,
            world,
            shared,
            host_id,
            dir,
            drag: Default::default(),
            was_down: Cell::new(false),
            outbox: Default::default(),
        };
        ed.step();
        Ok(ed)
    }

    pub fn project<R>(&self, f: impl FnOnce(&mut Project) -> R) -> R {
        f(&mut self.shared.project.borrow_mut())
    }

    fn bump(&self) {
        self.shared.extra.set(self.shared.extra.get() + 1);
    }

    /// Writes the scripts and the scene next to each other.
    pub fn save(&self) -> Result<String, String> {
        let dir = self.dir.clone().unwrap_or_else(|| PathBuf::from("."));
        let mut p = self.shared.project.borrow_mut();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for (name, text) in &p.sources {
            std::fs::write(dir.join(format!("{name}.py")), text).map_err(|e| e.to_string())?;
        }
        std::fs::write(dir.join("scene.sage"), p.doc.scene.write()).map_err(|e| e.to_string())?;
        if !p.settings.is_empty() {
            std::fs::write(dir.join("settings.sage"), p.settings_text())
                .map_err(|e| e.to_string())?;
        }
        p.doc.mark_saved();
        Ok(format!("saved to {}", dir.display()))
    }

    fn handle_requests(&self) {
        let requests: Vec<String> = self.shared.requests.borrow_mut().drain(..).collect();
        for r in requests {
            let result = {
                let mut p = self.shared.project.borrow_mut();
                match r.as_str() {
                    "play" => p.play().map(|_| "playing".to_string()),
                    "stop" => {
                        self.shared.paused.set(false);
                        p.stop().map(|_| "stopped".to_string())
                    }
                    "pause" => {
                        self.shared.paused.set(!self.shared.paused.get());
                        Ok(if self.shared.paused.get() {
                            "paused"
                        } else {
                            "running"
                        }
                        .to_string())
                    }
                    "reload" => p.reload().map(|_| "reloaded".to_string()),
                    "step" => {
                        if p.mode == Mode::Edit {
                            p.play()
                                .and_then(|_| p.step(1))
                                .map(|_| "stepped".to_string())
                        } else {
                            p.step(1).map(|_| "stepped".to_string())
                        }
                    }
                    "save" => {
                        drop(p);
                        self.save()
                    }
                    other => Err(format!("unknown request {other}")),
                }
            };
            *self.shared.log.borrow_mut() = match result {
                Ok(m) => m,
                Err(e) => e,
            };
            self.bump();
        }
    }

    /// Runs the part of a tick that is not the UI: requests, play, the
    /// scene viewport and what is done with the pointer over it.
    fn host_step(&self) {
        self.handle_requests();
        {
            let mut p = self.shared.project.borrow_mut();
            if p.mode == Mode::Play {
                if self.shared.paused.get() {
                    // Held: the game does not tick (STEP still advances it).
                } else if p.step(1).is_err() {
                    let e = p.error.clone().unwrap_or_default();
                    *self.shared.log.borrow_mut() = e;
                    drop(p);
                    self.bump();
                }
            } else {
                if p.stale && p.error.is_none() && self.shared.requests.borrow().is_empty() {
                    // Live reload: new rules take effect without being asked.
                    let _ = p.reload();
                    drop(p);
                    self.bump();
                } else if let Err(e) = p.sync() {
                    *self.shared.log.borrow_mut() = e;
                }
            }
        }
        self.viewport();
    }

    fn viewport(&self) {
        let mut view = None;
        self.world.new_query::<&UiViewport>().each(|v| {
            if v.source == SCENE_VIEW {
                view = Some(*v);
            }
        });
        let mut changed = false;
        if let Some(v) = view {
            let down = v.down & 1 != 0;
            let mut p = self.shared.project.borrow_mut();
            let editing = p.mode == Mode::Edit;
            if v.over != 0 && down && !self.was_down.get() && editing {
                // Pick the topmost entity under the pointer.
                let hit = p
                    .doc
                    .scene
                    .entities
                    .iter()
                    .rev()
                    .filter_map(|e| position_of(e).map(|(c, pos)| (e, c, pos)))
                    .find(|(e, _, pos)| {
                        let h = half_of(e);
                        (v.x - pos[0]).abs() <= h[0] && (v.y - pos[1]).abs() <= h[1]
                    })
                    .map(|(e, c, pos)| (e.sid, c.path.clone(), pos));
                match hit {
                    Some((sid, component, pos)) => {
                        p.doc.select(Some(sid));
                        *self.drag.borrow_mut() = Some(Drag {
                            sid,
                            component,
                            grab: [v.x - pos[0], v.y - pos[1]],
                        });
                    }
                    None => p.doc.select(None),
                }
                changed = true;
            }
            if down {
                if let Some(d) = self.drag.borrow().clone() {
                    let (x, y) = (
                        format!("{:.1}", v.x - d.grab[0]),
                        format!("{:.1}", v.y - d.grab[1]),
                    );
                    let _ = p
                        .doc
                        .set_fields(d.sid, &d.component, &[("x", &x), ("y", &y)]);
                }
            } else if self.was_down.get() {
                p.doc.end_gesture();
                *self.drag.borrow_mut() = None;
            }
            self.was_down.set(down);
        }
        let (scene, selected, tick) = {
            let p = self.shared.project.borrow();
            (p.shown_scene(), p.doc.selection, p.tick)
        };
        sage_ui::set_viewport_frame(&self.ecs, SCENE_VIEW, scene_frame(&scene, selected, tick));
        if changed {
            self.bump();
        }
    }

    /// One UI tick, then the host's part.
    fn step(&self) {
        self.world.progress();
        self.host_step();
        self.refresh_views();
        if let Some(fault) = self.ecs.fault() {
            panic!("editor script error: {fault}");
        }
    }

    /// Rebuilds the views that changed (`refresh_views` in the script).
    fn refresh_views(&self) {
        Python::attach(|py| {
            let f = self
                .script
                .globals(py)
                .get_item("refresh_views")
                .expect("globals")
                .expect("the script defines refresh_views");
            if let Err(e) = f.call0() {
                panic!("editor script error: {}", sage_script::describe(py, &e));
            }
        });
    }

    // ---- driving the UI the way a window does (also used by tests) -------

    pub fn input(&self, i: &UiTickInput) {
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

    pub fn idle(&self, n: u32) {
        for _ in 0..n {
            self.input(&UiTickInput::default());
        }
    }

    pub fn click_at(&self, p: [f32; 2]) {
        for down in [true, false] {
            self.input(&UiTickInput {
                pointer: Some(p),
                down,
                ..UiTickInput::default()
            });
        }
        self.idle(1);
    }

    /// Presses at `from`, moves to `to`, releases.
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

    pub fn type_text(&self, text: &str) {
        for c in text.chars() {
            self.input(&UiTickInput {
                ch: Some(c),
                ..UiTickInput::default()
            });
            self.input(&UiTickInput::default());
        }
    }

    pub fn press(&self, bits: u32) {
        self.input(&UiTickInput {
            nav: bits,
            ..UiTickInput::default()
        });
        self.input(&UiTickInput::default());
    }

    fn namespace<'py>(&self, py: Python<'py>) -> Bound<'py, pyo3::types::PyDict> {
        self.script.globals(py).copy().expect("copy globals")
    }

    /// Evaluates a Python expression in the editor script's names.
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

    /// Runs Python statements in the editor script's names.
    pub fn run(&self, code: &str) {
        Python::attach(|py| {
            let code = std::ffi::CString::new(code).unwrap();
            let ns = self.namespace(py);
            py.run(&code, Some(&ns), None)
                .unwrap_or_else(|e| panic!("{}", sage_script::describe(py, &e)));
        });
    }

    /// The computed rectangle of the entity `expr` evaluates to: x, y, w, h.
    pub fn rect(&self, expr: &str) -> [f32; 4] {
        let sid = self.eval::<u64>(&format!("({expr}).sid"));
        let e = self.ecs.entity_of_sid(sid).expect("entity is in the world");
        let id = self.ecs.lookup_path("sage.ui.Rect");
        ["x", "y", "w", "h"].map(|f| self.ecs.field(e, id, f).unwrap() as f32)
    }

    pub fn center(&self, expr: &str) -> [f32; 2] {
        let [x, y, w, h] = self.rect(expr);
        [x + w / 2.0, y + h / 2.0]
    }

    pub fn click(&self, expr: &str) {
        self.click_at(self.center(expr));
    }

    /// What a screen reader is told about the editor, as indented text.
    pub fn access_outline(&self) -> String {
        sage_ui::access_outline(&self.world, &self.ecs)
    }

    /// Presses a key chord (`key` as `ui.parse_chord` gives it) for one tick.
    pub fn chord(&self, key: u32, mods: u32) {
        self.input(&UiTickInput {
            chord: Some([key, mods]),
            ..UiTickInput::default()
        });
        self.idle(2);
    }

    pub fn ui_frame(&self) -> UiFrame {
        sage_ui::frame(&self.world, &self.ecs)
    }

    /// Everything the UI shows as text, in draw order.
    pub fn texts(&self) -> Vec<String> {
        self.ui_frame()
            .draws
            .into_iter()
            .filter_map(|d| match d {
                sage_core::host::UiDraw::Text { text, .. } => Some(text.replace('\n', " ")),
                _ => None,
            })
            .collect()
    }

    pub fn render(&self, size: [u32; 2]) -> Image {
        let font = BitmapFont::builtin();
        let mut res = UiResources::new(2);
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

impl UiApp for Editor {
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
}
