//! A playground for the UI module: every widget, themed by style prefabs.
//!
//! `scripts/theme.py` is the palette and the prefabs, `scripts/playground.py`
//! is the screen and its behavior. This crate wires them to a world, feeds
//! the UI's input singletons from a window (or from a script of inputs, for
//! tests and screenshots), and turns the UI into pixels.

use flecs_ecs::prelude::*;
use pyo3::prelude::*;
use sage_assets::{BitmapFont, Image};
use sage_core::host::{NAV_ACCEPT, NAV_NEXT, UiFrame};
use sage_ecs_py::Ecs;
use sage_platform::sprite::{UiResources, compose_ui, rasterize};
use sage_platform::ui_window::{UiApp, UiTickInput};
use sage_script::Script;
use sage_ui::ecs::{UiNav, UiPointer, UiScreen, UiTextEntry};

pub const THEME: &str = include_str!("../scripts/theme.py");
pub const SCRIPT: &str = include_str!("../scripts/playground.py");

/// The pages loaded after the main script, by name. A page registers
/// itself with `world.add_page`.
pub const PAGES: &[(&str, &str)] = &[
    ("style", include_str!("../scripts/pages/style.py")),
    ("popups", include_str!("../scripts/pages/popups.py")),
];

pub mod assets;

/// The UI's size in UI units. A window shows it at a whole-number scale:
/// 960x540 pixels at 2, 1440x810 at 3.
pub const SCREEN: [f32; 2] = [480.0, 270.0];

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
}

impl Playground {
    pub fn new() -> Result<Playground, String> {
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
        };
        p.step();
        Ok(p)
    }

    fn step(&self) {
        self.world.progress();
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

    /// One tick with the given input, the way the window loop does it.
    pub fn input(&self, i: &UiTickInput) {
        self.world.set(UiPointer {
            x: i.pointer.map_or(0.0, |p| p[0]),
            y: i.pointer.map_or(0.0, |p| p[1]),
            present: i.pointer.is_some() as u32,
            down: i.down as u32 | (i.secondary as u32) << 1,
            wheel: i.wheel,
        });
        self.world.set(UiNav { bits: i.nav });
        self.world.set(UiTextEntry {
            ch: i.ch.map_or(0, |c| c as u32),
        });
        self.step();
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

    pub fn ui_frame(&self) -> UiFrame {
        sage_ui::frame(&self.world, &self.ecs)
    }

    /// Renders the current UI into an image of `size` pixels with the CPU
    /// reference rasterizer (the same batch the GPU draws).
    pub fn render(&self, size: [u32; 2]) -> Image {
        let font = BitmapFont::builtin();
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

    fn images(&self) -> Vec<(u64, Image)> {
        assets::all()
            .into_iter()
            .map(|(_, id, i)| (id, i))
            .collect()
    }
}
