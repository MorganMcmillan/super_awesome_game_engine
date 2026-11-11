//! A settings menu built from the UI module's widgets (`scripts/menu.py`),
//! driven through the same pointer, navigation and text singletons a game
//! would set, and rendered with the sprite path's CPU reference rasterizer.
//!
//! The windowed path is not wired: the platform layer has no mouse input yet
//! (docs/milestone-2.md), so this example renders frames instead.

use flecs_ecs::prelude::*;
use pyo3::prelude::*;
use sage_assets::{BitmapFont, Image};
use sage_core::host::UiFrame;
use sage_ecs_py::Ecs;
use sage_platform::sprite::{compose_ui, rasterize};
use sage_script::Script;
use sage_ui::ecs::{UiNav, UiPointer, UiScreen, UiTextEntry};

pub const SCRIPT: &str = include_str!("../scripts/menu.py");

/// The UI size in UI units; a window shows it at a whole-number scale.
pub const SCREEN: [f32; 2] = [160.0, 144.0];

/// Fields drop in order: the binding, the scripts, then the world.
pub struct Demo {
    pub ecs: Ecs,
    _kit: Script,
    menu: Script,
    pub world: World,
}

impl Demo {
    pub fn new() -> Result<Demo, String> {
        let world = World::new();
        let ecs = Ecs::new(&world, 1, "menu").map_err(|e| e.0)?;
        sage_ui::setup(&world, &ecs)?;
        world.set(UiScreen {
            w: SCREEN[0],
            h: SCREEN[1],
        });
        let kit = ecs.load("ui", sage_ui::WIDGETS).map_err(|e| e.0)?;
        let menu = ecs.load("menu", SCRIPT).map_err(|e| e.0)?;
        let demo = Demo {
            ecs,
            _kit: kit,
            menu,
            world,
        };
        demo.tick();
        Ok(demo)
    }

    pub fn tick(&self) {
        self.world.progress();
        if let Some(fault) = self.ecs.fault() {
            panic!("script error: {fault}");
        }
    }

    /// The entity the script holds in the Python variable `var` (`play`,
    /// `music`, `volume`, `name`, `status` or `panel`).
    fn entity(&self, var: &str) -> u64 {
        let sid = Python::attach(|py| {
            let code = std::ffi::CString::new(format!("{var}.sid")).unwrap();
            py.eval(&code, Some(self.menu.globals(py)), None)
                .and_then(|v| v.extract::<u64>())
                .unwrap_or_else(|e| panic!("{var}: {}", sage_script::describe(py, &e)))
        });
        self.ecs.entity_of_sid(sid).expect("no such widget")
    }

    /// The computed rectangle of the widget held in the variable `var`.
    pub fn rect(&self, var: &str) -> [f32; 4] {
        let e = self.entity(var);
        let id = self.ecs.lookup_path("sage.ui.Rect");
        ["x", "y", "w", "h"].map(|f| self.ecs.field(e, id, f).unwrap() as f32)
    }

    pub fn center(&self, var: &str) -> [f32; 2] {
        let [x, y, w, h] = self.rect(var);
        [x + w / 2.0, y + h / 2.0]
    }

    pub fn pointer(&self, p: [f32; 2], down: bool) {
        self.world.set(UiPointer {
            x: p[0],
            y: p[1],
            present: 1,
            down: down as u32,
            wheel: 0.0,
        });
    }

    pub fn press_at(&self, p: [f32; 2]) {
        self.pointer(p, true);
        self.tick();
        self.pointer(p, false);
        self.tick();
    }

    pub fn type_text(&self, text: &str) {
        for c in text.chars() {
            self.world.set(UiTextEntry { ch: c as u32 });
            self.tick();
            self.world.set(UiTextEntry::default());
            self.tick();
        }
    }

    pub fn nav(&self, bits: u32) {
        self.world.set(UiNav { bits });
        self.tick();
        self.world.set(UiNav::default());
        self.tick();
    }

    /// The text of the status line.
    pub fn status(&self) -> String {
        let e = self.entity("status");
        let id = self.ecs.lookup_path("sage.ui.Label");
        let text = self.ecs.field_u64(e, id, "text").unwrap();
        self.ecs
            .strings()
            .borrow()
            .get(text)
            .unwrap_or("")
            .to_string()
    }

    pub fn frame(&self) -> UiFrame {
        sage_ui::frame(&self.world, &self.ecs)
    }

    /// Renders the current UI into an image of `size` pixels.
    pub fn render(&self, size: [u32; 2]) -> Image {
        let font = BitmapFont::builtin();
        let batch = compose_ui(&self.frame(), [size[0] as f32, size[1] as f32], &font);
        let textures = [Image::solid(1, 1, [255; 4]), font.atlas.clone()];
        let px = rasterize(&batch, &textures, [12, 14, 18, 255], size);
        let mut img = Image::solid(size[0], size[1], [0, 0, 0, 255]);
        for (i, p) in px.iter().enumerate() {
            img.set_pixel(i as u32 % size[0], i as u32 / size[0], *p);
        }
        img
    }

    /// Plays a short session: moves the volume, turns the music on, types a
    /// name and hovers over Play.
    pub fn play_session(&self) {
        let [x, y, w, h] = self.rect("volume");
        self.press_at([x + w * 0.7, y + h / 2.0]);
        self.press_at(self.center("music"));
        self.press_at(self.center("name"));
        self.type_text("Ada");
        self.pointer(self.center("play"), false);
        self.tick();
    }
}
