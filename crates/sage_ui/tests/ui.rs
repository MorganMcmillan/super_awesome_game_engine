//! The UI module end to end: widgets written in Python, driven through the
//! pointer, navigation and text singletons, observed through events.

use std::collections::BTreeMap;

use flecs_ecs::prelude::*;
use pyo3::prelude::*;
use sage_core::StateHasher;
use sage_core::host::UiDraw;
use sage_ecs_py::{Ecs, Policy};
use sage_script::Script;
use sage_ui::ecs::{UiNav, UiPointer, UiTextEntry};
use sage_ui::interact::{NAV_ACCEPT, NAV_BACKSPACE, NAV_NEXT, NAV_RIGHT};

/// Fields drop in order: the binding, the scripts, then the world.
struct Ui {
    ecs: Ecs,
    script: Script,
    _kit: Script,
    world: World,
}

const MENU: &str = r#"
import sage
from sage import OnUpdate
from sage.ui import Clicked, Toggled, Changed, TextChanged, Submitted, Focused, Rect, Slider, Toggle, Label

ui = world.ui
ui.start()
log = []

root = ui.panel(width=100, height=90, padding=4, gap=2, fill=0x202020ff)
title = ui.text(root, "Menu")
play = ui.button(root, "Play", width=ui.fill())
music = ui.toggle(root, "Music", on=False)
volume = ui.slider(root, value=2, low=0, high=10, step=1, width=ui.fill())
name = ui.text_input(root, "", width=ui.fill())

@world.observer(Clicked)
def clicked(c: Clicked):
    log.append(("click", c.target))

@world.observer(Toggled)
def toggled(t: Toggled):
    log.append(("toggle", t.target, t.on))

@world.observer(Changed)
def changed(c: Changed):
    log.append(("change", c.target, c.value))

@world.observer(TextChanged)
def text_changed(t: TextChanged):
    log.append(("text", t.target))

@world.observer(Submitted)
def submitted(s: Submitted):
    log.append(("submit", s.target))

def center(e):
    r = e.get(Rect)
    return (r.x + r.w / 2, r.y + r.h / 2)

def text_of(e):
    return world.string(e.get(Label).text)
"#;

impl Ui {
    fn new(src: &str) -> Ui {
        let world = World::new();
        let ecs = Ecs::new(&world, 3, "game").unwrap();
        sage_ui::setup(&world, &ecs).unwrap();
        let kit = ecs.load("ui", sage_ui::WIDGETS).unwrap();
        let script = ecs.load("game", src).unwrap();
        Ui {
            ecs,
            script,
            _kit: kit,
            world,
        }
    }

    fn tick(&self) {
        self.world.progress();
        if let Some(f) = self.ecs.fault() {
            panic!("{f}");
        }
    }

    fn eval<T: for<'py> FromPyObjectOwned<'py>>(&self, expr: &str) -> T {
        Python::attach(|py| {
            let code = std::ffi::CString::new(expr).unwrap();
            py.eval(&code, Some(self.script.globals(py)), None)
                .unwrap_or_else(|e| panic!("{expr}: {}", sage_script::describe(py, &e)))
                .extract()
                .unwrap_or_else(|_| panic!("{expr}: wrong type"))
        })
    }

    fn pointer(&self, x: f32, y: f32, down: bool) {
        self.world.set(UiPointer {
            x,
            y,
            present: 1,
            down: down as u32,
        });
    }

    fn no_pointer(&self) {
        self.world.set(UiPointer::default());
    }

    fn nav(&self, bits: u32) {
        self.world.set(UiNav { bits });
    }

    fn typed(&self, c: char) {
        self.world.set(UiTextEntry { ch: c as u32 });
    }

    /// Presses and releases the pointer at the center of `entity`.
    fn click(&self, entity: &str) {
        let (x, y) = self.eval::<(f32, f32)>(&format!("center({entity})"));
        self.pointer(x, y, true);
        self.tick();
        self.pointer(x, y, false);
        self.tick();
    }

    fn take_log(&self) -> String {
        let s = self.eval::<String>("repr(log)");
        Python::attach(|py| {
            py.run(c"log.clear()", Some(self.script.globals(py)), None)
                .unwrap()
        });
        s
    }

    fn hash(&self) -> u64 {
        let mut h = StateHasher::new();
        self.ecs.hash_into(&mut h);
        h.finish()
    }
}

#[test]
fn layout_is_computed_into_rect_components() {
    let ui = Ui::new(MENU);
    ui.tick();
    let r = |e: &str| {
        ui.eval::<(f32, f32, f32, f32)>(&format!("(lambda r: (r.x, r.y, r.w, r.h))({e}.get(Rect))"))
    };
    assert_eq!(r("root"), (0.0, 0.0, 100.0, 90.0));
    // Children fill the width inside the padding of 4 and stack with gap 2.
    let (x, y, w, h) = r("play");
    assert_eq!((x, w), (4.0, 92.0));
    let (_, title_y, _, title_h) = r("title");
    assert_eq!(title_y, 4.0);
    assert_eq!(y, title_y + title_h + 2.0, "the button follows the title");
    assert_eq!(h, 7.0 + 6.0, "a button is its text plus padding");
    let (mx, my, _, _) = r("music");
    assert_eq!(mx, 4.0);
    assert!(my > y);
}

#[test]
fn clicking_a_button_emits_one_clicked_event() {
    let ui = Ui::new(MENU);
    ui.tick();
    ui.click("play");
    let log = ui.take_log();
    assert!(log.contains("('click',"), "{log}");
    assert_eq!(log.matches("click").count(), 1, "{log}");
    // The target is the button's StableId.
    ui.click("play");
    let sid = ui.eval::<u64>("play.sid");
    assert!(ui.take_log().contains(&format!("('click', {sid})")));
    // No pointer, no events.
    ui.no_pointer();
    ui.tick();
    assert_eq!(ui.take_log(), "[]");
}

#[test]
fn toggles_sliders_and_text_inputs_hold_their_state_in_components() {
    let ui = Ui::new(MENU);
    ui.tick();
    ui.click("music");
    assert!(ui.take_log().contains("'toggle'"));
    assert_eq!(ui.eval::<u32>("music.get(Toggle).on"), 1);
    // Press in the middle of the slider: 0..10, step 1, so 5.
    let (x, y) = ui.eval::<(f32, f32)>("center(volume)");
    ui.pointer(x, y, true);
    ui.tick();
    ui.pointer(x, y, false);
    ui.tick();
    assert_eq!(ui.eval::<f32>("volume.get(Slider).value"), 5.0);
    assert!(ui.take_log().contains("'change'"));
    // Focus the text field with the pointer, then type.
    ui.click("name");
    ui.take_log();
    for c in "Ada".chars() {
        ui.typed(c);
        ui.tick();
        ui.typed('\0');
        ui.tick();
    }
    assert_eq!(ui.eval::<String>("text_of(name)"), "Ada");
    ui.nav(NAV_BACKSPACE);
    ui.tick();
    ui.nav(0);
    ui.tick();
    assert_eq!(ui.eval::<String>("text_of(name)"), "Ad");
    ui.nav(NAV_ACCEPT);
    ui.tick();
    assert!(ui.take_log().contains("'submit'"));
}

#[test]
fn the_keyboard_alone_can_drive_the_menu() {
    let ui = Ui::new(MENU);
    ui.no_pointer();
    ui.tick();
    let press = |bits: u32| {
        ui.nav(bits);
        ui.tick();
        ui.nav(0);
        ui.tick();
    };
    press(NAV_NEXT); // play
    press(NAV_ACCEPT);
    assert!(ui.take_log().contains("'click'"));
    press(NAV_NEXT); // music
    press(NAV_ACCEPT);
    assert_eq!(ui.eval::<u32>("music.get(Toggle).on"), 1);
    press(NAV_NEXT); // volume
    press(NAV_RIGHT);
    assert_eq!(ui.eval::<f32>("volume.get(Slider).value"), 3.0);
}

#[test]
fn ui_state_is_hashed_and_a_save_game_restores_it() {
    let a = Ui::new(MENU);
    a.tick();
    let idle = a.hash();
    a.click("music");
    assert_ne!(a.hash(), idle, "toggling changes the world hash");
    // Save in the middle of a press: the interaction state is saved too.
    let (x, y) = a.eval::<(f32, f32)>("center(volume)");
    a.pointer(x, y, true);
    a.tick();
    let text = a.ecs.save_game(BTreeMap::new());
    let at_save = a.hash();
    let mut later = vec![];
    for step in 0..6 {
        a.pointer(x + step as f32, y, step < 3);
        a.tick();
        later.push(a.hash());
    }
    let b = Ui::new(MENU);
    for _ in 0..4 {
        b.tick();
    }
    b.ecs.load_game(&text, Policy::Strict).unwrap();
    assert_eq!(b.hash(), at_save);
    b.pointer(x, y, true); // the host's pointer is input, not saved state
    for (step, want) in later.iter().enumerate() {
        b.pointer(x + step as f32, y, step < 3);
        b.tick();
        assert_eq!(b.hash(), *want, "tick {} after loading", step + 1);
    }
    // Two identical runs agree tick by tick.
    let run = || {
        let g = Ui::new(MENU);
        let mut hashes = vec![];
        for step in 0..8 {
            g.pointer(30.0, 10.0 + step as f32, step % 2 == 0);
            g.tick();
            hashes.push(g.hash());
        }
        hashes
    };
    assert_eq!(run(), run());
}

#[test]
fn the_frame_is_plain_draw_data_and_the_module_can_be_paused() {
    let ui = Ui::new(MENU);
    ui.tick();
    let f = sage_ui::frame(&ui.world, &ui.ecs);
    assert_eq!(f.size, [320.0, 180.0]);
    let texts: Vec<_> = f
        .draws
        .iter()
        .filter_map(|d| match d {
            UiDraw::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["Menu", "Play", "Music"]);
    // The first draw is the root panel's fill.
    assert_eq!(
        f.draws[0],
        UiDraw::Rect {
            pos: [0.0, 0.0],
            size: [100.0, 90.0],
            color: [0x20, 0x20, 0x20, 0xff]
        }
    );
    // Disabling the flecs module pauses layout and interaction.
    Python::attach(|py| {
        let code = std::ffi::CString::new("world.lookup_path('game.ui').disable()").unwrap();
        py.run(&code, Some(ui.script.globals(py)), None).unwrap();
    });
    let (x, y) = ui.eval::<(f32, f32)>("center(play)");
    ui.pointer(x, y, true);
    ui.tick();
    ui.pointer(x, y, false);
    ui.tick();
    assert_eq!(ui.take_log(), "[]", "a paused UI emits nothing");
}

/// A theme as prefabs: `Primary` and `Danger` derive from the kit's button.
const THEMED: &str = r#"
import sage
from sage.ui import Paint, TextStyle, Label

ui = world.ui
ui.start()
Primary = world.prefab("Primary", base=ui.Button)
Primary.set(Paint(0xE8741AFF, 0, 0.0, 0xFFFFFFFF))
Danger = world.prefab("Danger", base=ui.Button)
Danger.set(Paint(0xC0392BFF, 0, 0.0, 0xFFFFFFFF)).set(TextStyle(0xFFFFFFFF, 2.0, 0))

root = ui.panel(width=120, height=60, gap=2)
a = ui.add(Primary, root, text="A", width=40)
b = ui.add(Primary, root, text="B", width=40)
c = ui.add(Danger, root, text="C", width=40)
own = ui.add(Primary, root, text="D", width=40, fill=0x112233FF)

def fill_of(e):
    return e.get(Paint).fill
def scale_of(e):
    return e.get(TextStyle).scale
"#;

#[test]
fn editing_a_style_prefab_restyles_its_instances() {
    let ui = Ui::new(THEMED);
    ui.tick();
    let fill = |e: &str| ui.eval::<u32>(&format!("fill_of({e})"));
    assert_eq!(
        fill("a"),
        0xE874_1AFF,
        "an instance reads its prefab's style"
    );
    assert_eq!(fill("c"), 0xC039_2BFF);
    assert_eq!(
        ui.eval::<f32>("scale_of(c)"),
        2.0,
        "text style is inherited too"
    );
    assert_eq!(
        ui.eval::<f32>("scale_of(a)"),
        1.0,
        "Primary inherits the base text style"
    );
    // Edit the prefab: instances that did not override the group follow.
    Python::attach(|py| {
        py.run(
            c"Primary.set(Paint(0x2E86ABFF, 0, 0.0, 0xFFFFFFFF))",
            Some(ui.script.globals(py)),
            None,
        )
        .unwrap();
    });
    ui.tick();
    assert_eq!(fill("a"), 0x2E86_ABFF);
    assert_eq!(fill("b"), 0x2E86_ABFF);
    assert_eq!(fill("c"), 0xC039_2BFF, "a different prefab is unaffected");
    assert_eq!(
        fill("own"),
        0x1122_33FF,
        "an instance that set its own Paint keeps it"
    );
    // And what is drawn follows: the first button's fill is the new color.
    let f = sage_ui::frame(&ui.world, &ui.ecs);
    let colors: Vec<[u8; 4]> = f
        .draws
        .iter()
        .filter_map(|d| match d {
            UiDraw::Rect { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert!(colors.contains(&[0x2E, 0x86, 0xAB, 0xFF]), "{colors:?}");
    assert!(
        !colors.contains(&[0xE8, 0x74, 0x1A, 0xFF]),
        "the old color is gone"
    );
    // The hash covers the edit, and a save restores the inherited look.
    let text = ui.ecs.save_game(Default::default());
    let other = Ui::new(THEMED);
    other.ecs.load_game(&text, Policy::Strict).unwrap();
    other.tick();
    assert_eq!(other.eval::<u32>("fill_of(a)"), 0x2E86_ABFF);
    assert_eq!(other.hash(), ui.hash());
}
