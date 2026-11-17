//! A playground for the UI module: every widget, themed by style prefabs.
//!
//! `scripts/theme.py` is the palette and the prefabs, `scripts/playground.py`
//! is the screen and its behavior. This crate wires them to a world, feeds
//! the UI's input singletons from a window (or from a script of inputs, for
//! tests and screenshots), and turns the UI into pixels.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use flecs_ecs::prelude::*;
use sage_app::{UiHost, UiHosted};
use sage_assets::{AssetId, BitmapFont, Image, Sound};
use sage_core::host::{NAV_ACCEPT, NAV_NEXT};
use sage_platform::media::{MediaState, MediaVoice};
use sage_platform::mixer::Mixer;

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

pub struct Playground {
    ui: UiHost,
    /// A TrueType face to draw text with instead of the built-in one (the
    /// bytes, and whether it is shaped and proportional; rasterized for the
    /// scale it is drawn at).
    face: RefCell<Option<(Vec<u8>, bool)>>,
    /// Audio for the MEDIA page: sounds by asset id, one voice per player, and
    /// the mixer they play on. Presentation: none of it is world state.
    sounds: BTreeMap<u64, Rc<Sound>>,
    voices: RefCell<BTreeMap<u64, MediaVoice>>,
    mixer: RefCell<Mixer>,
    /// The simulation embedded in the CANVAS page's viewport.
    pub bouncer: RefCell<Bouncer>,
}

impl std::ops::Deref for Playground {
    type Target = UiHost;
    fn deref(&self) -> &UiHost {
        &self.ui
    }
}

impl UiHosted for Playground {
    fn ui(&self) -> &UiHost {
        &self.ui
    }

    /// The embedded simulation: forward the pointer the viewport reports,
    /// step it, and give the viewport what it drew. Then the media players.
    fn after_ui(&self) {
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
    }

    fn font(&self, scale: u32) -> Option<BitmapFont> {
        let face = self.face.borrow();
        let (bytes, proportional) = face.as_ref()?;
        if *proportional {
            BitmapFont::from_ttf_proportional(bytes, scale).ok()
        } else {
            BitmapFont::from_ttf(bytes, scale).ok()
        }
    }

    fn images(&self) -> Vec<(u64, Image)> {
        assets::all()
            .into_iter()
            .map(|(_, id, i)| (id, i))
            .collect()
    }

    fn clear_color(&self) -> [u8; 4] {
        CLEAR
    }

    fn mix_audio(&self, frames: usize) -> Option<Vec<f32>> {
        Some(self.audio(frames))
    }
}

impl Playground {
    pub fn new() -> Result<Playground, String> {
        // No proportional face until one is asked for (the metrics are per
        // thread, and a test thread may have had one).
        sage_core::host::set_text_metrics(None);
        let mut ui = UiHost::new("playground", SCREEN)?;
        ui.load("theme", THEME)?;
        for (_, id, image) in assets::all() {
            sage_ui::register_image(&ui.ecs, id, [image.width, image.height]);
        }
        ui.load("playground", SCRIPT)?;
        for (name, source) in PAGES {
            ui.load(name, source)?;
        }
        let p = Playground {
            ui,
            face: Default::default(),
            sounds: [(AssetId::of_path(assets::TONE).0, Rc::new(assets::tone()))].into(),
            voices: Default::default(),
            mixer: RefCell::new(Mixer::new(sage_platform::audio::OUT_RATE)),
            bouncer: Default::default(),
        };
        sage_ui::set_viewport_frame(&p.ecs, BOUNCER_VIEW, p.bouncer.borrow().frame());
        p.step();
        Ok(p)
    }

    pub fn tab(&self) {
        self.press(NAV_NEXT);
    }

    pub fn accept(&self) {
        self.press(NAV_ACCEPT);
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
                MediaState {
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
}
