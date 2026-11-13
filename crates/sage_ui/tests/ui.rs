//! The UI module end to end: widgets written in Python, driven through the
//! pointer, navigation and text singletons, observed through events.

use std::collections::BTreeMap;

use flecs_ecs::prelude::*;
use pyo3::prelude::*;
use sage_core::StateHasher;
use sage_core::host::UiDraw;
use sage_ecs_py::{Ecs, Policy};
use sage_script::Script;
use sage_ui::ecs::{UiChord, UiNav, UiPointer, UiTextEntry};
use sage_ui::interact::{NAV_ACCEPT, NAV_BACKSPACE, NAV_NEXT, NAV_RIGHT};

/// Fields drop in order: the binding, the scripts, then the world.
struct Ui {
    ecs: Ecs,
    script: Script,
    _kit: Vec<Script>,
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
        let kit = sage_ui::load_kit(&ecs).unwrap();
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
            wheel: 0.0,
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
fn a_command_is_a_shortcut_entity_with_nothing_to_click() {
    let ui = Ui::new(MENU);
    ui.eval::<bool>("ui.command('Ctrl+K', name='palette') is not None");
    ui.tick();
    let chord = |key: u32, mods: u32| ui.world.set(UiChord { key, mods });
    chord(b'K' as u32, 1);
    ui.tick();
    let sid = ui.eval::<u64>("world.lookup_path('palette').sid");
    assert_eq!(ui.take_log(), format!("[('click', {sid})]"));
    // The host sets the chord for one tick; another chord does nothing.
    chord(b'J' as u32, 1);
    ui.tick();
    assert_eq!(ui.take_log(), "[]");
    // A modal blocks it, since the command is outside the dialog.
    ui.eval::<bool>("ui.open(ui.dialog('Sure?')[0]) is None or True");
    ui.tick();
    ui.take_log();
    chord(b'K' as u32, 1);
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

impl Ui {
    fn wheel(&self, x: f32, y: f32, notches: f32) {
        self.world.set(UiPointer {
            x,
            y,
            present: 1,
            down: 0,
            wheel: notches,
        });
        self.tick();
        self.pointer(x, y, false);
        self.tick();
    }
}

const LIST: &str = r#"
import sage
from sage.ui import Rect, Scroll, Slider, Caret, Label, TextStyle, Clicked, Interaction

ui = world.ui
ui.start()
log = []
root = ui.panel(width=100, height=100, gap=0)
area = ui.scroll(root, width=ui.fill(), height=40)
rows = [ui.button(area, f"row {k}", width=ui.fill(), height=20) for k in range(6)]
note = ui.text(root, "the quick brown fox jumps over", wrap=True, width=ui.fill())
bar = ui.meter(root, value=3, low=0, high=10, width=ui.fill())
field = ui.text_input(root, "hello", width=ui.fill())

@world.observer(Clicked)
def clicked(c: Clicked):
    log.append(c.target)

def center(e):
    r = e.get(Rect)
    return (r.x + r.w / 2, r.y + r.h / 2)

def y_of(e):
    return e.get(Rect).y
def h_of(e):
    return e.get(Rect).h
"#;

#[test]
fn a_scroll_area_scrolls_with_the_wheel_and_clips_what_it_hides() {
    let ui = Ui::new(LIST);
    ui.tick();
    assert_eq!(ui.eval::<f32>("area.get(Scroll).offset"), 0.0);
    assert_eq!(
        ui.eval::<f32>("y_of(rows[5])"),
        100.0,
        "laid out below the view"
    );
    let (x, y) = ui.eval::<(f32, f32)>("center(area)");
    ui.wheel(x, y, 2.0);
    assert_eq!(ui.eval::<f32>("area.get(Scroll).offset"), 24.0);
    assert_eq!(ui.eval::<f32>("y_of(rows[0])"), -24.0, "content moved up");
    // A click on the visible part of the scrolled content reaches the right
    // row (the second, which is cut off at the top).
    let sid = ui.eval::<u64>("rows[1].sid");
    ui.pointer(5.0, 5.0, true);
    ui.tick();
    ui.pointer(5.0, 5.0, false);
    ui.tick();
    assert!(ui.take_log().contains(&sid.to_string()));
    // Focusing a partly hidden row scrolled it fully into view: 24 -> 20.
    assert_eq!(ui.eval::<f32>("area.get(Scroll).offset"), 20.0);
    // The frame cuts the rows at the area and draws a scrollbar.
    let f = sage_ui::frame(&ui.world, &ui.ecs);
    let rects: Vec<_> = f
        .draws
        .iter()
        .filter_map(|d| match d {
            UiDraw::Rect { pos, size, .. } => Some((*pos, *size)),
            _ => None,
        })
        .collect();
    assert!(
        rects.iter().all(|(p, s)| !(p[1] < 40.0
            && p[1] + s[1] > 40.0
            && p[0] < 99.0
            && s[1] < 20.0
            && p[1] > 20.0)),
        "nothing straddles the area's bottom edge"
    );
    assert!(
        rects.iter().any(|(_, s)| s[0] == 2.0),
        "the scrollbar thumb: {:#?}",
        f.draws
    );
    // It is state: it hashes, and a save game restores the offset.
    let text = ui.ecs.save_game(Default::default());
    let other = Ui::new(LIST);
    other.ecs.load_game(&text, Policy::Strict).unwrap();
    assert_eq!(other.eval::<f32>("area.get(Scroll).offset"), 20.0);
    assert_eq!(other.hash(), ui.hash());
}

#[test]
fn wrapping_text_gets_taller_in_a_narrow_parent_and_meters_show_values() {
    let ui = Ui::new(LIST);
    ui.tick();
    // 100 wide: 11 columns need 65 units, so "the quick brown fox jumps over"
    // (30 chars, 16 columns at 100 wide) fits on two lines.
    let lines = ui.eval::<f32>("h_of(note) / 9.0");
    assert!(
        lines > 1.0 && lines < 3.0,
        "two lines of 7..9 units: {}",
        ui.eval::<f32>("h_of(note)")
    );
    let narrow = Ui::new(&LIST.replace("width=100, height=100", "width=50, height=100"));
    narrow.tick();
    assert!(
        narrow.eval::<f32>("h_of(note)") > ui.eval::<f32>("h_of(note)"),
        "narrower means taller"
    );
    let f = sage_ui::frame(&ui.world, &ui.ecs);
    let wrapped = f
        .draws
        .iter()
        .any(|d| matches!(d, UiDraw::Text { text, .. } if text.contains('\n')));
    assert!(wrapped, "the wrapped lines are what is drawn");
    // The meter: 3 of 10 fills 30% of its width, and ignores the pointer.
    let (x, y) = ui.eval::<(f32, f32)>("center(bar)");
    ui.pointer(x, y, true);
    ui.tick();
    ui.pointer(x, y, false);
    ui.tick();
    assert_eq!(ui.take_log(), "[]");
    assert_eq!(
        ui.eval::<u64>("world.get(Interaction).focus"),
        0,
        "a meter takes no focus"
    );
}

#[test]
fn the_caret_is_a_component_and_edits_happen_at_it() {
    let ui = Ui::new(LIST);
    ui.tick();
    let (x, y) = ui.eval::<(f32, f32)>("center(field)");
    // Click after two glyphs of "hello": the text starts 3 units in.
    let left = ui.eval::<f32>("field.get(Rect).x") + 3.0;
    let _ = x;
    ui.pointer(left + 6.0 * 2.0 + 1.0, y, true);
    ui.tick();
    ui.pointer(left + 6.0 * 2.0 + 1.0, y, false);
    ui.tick();
    assert_eq!(ui.eval::<u32>("field.get(Caret).pos"), 2);
    ui.typed('X');
    ui.tick();
    ui.typed('\0');
    ui.tick();
    assert_eq!(
        ui.eval::<String>("world.string(field.get(Label).text)"),
        "heXllo"
    );
    assert_eq!(ui.eval::<u32>("field.get(Caret).pos"), 3);
    ui.nav(sage_core::host::NAV_DELETE);
    ui.tick();
    ui.nav(0);
    ui.tick();
    assert_eq!(
        ui.eval::<String>("world.string(field.get(Label).text)"),
        "heXlo"
    );
}

const STYLED: &str = r#"
import sage
from sage.ui import Rect, Decor, Display, Shown, States, Transition, Paint, Image

ui = world.ui
ui.start()
root = ui.panel(width=200, height=120, gap=4, padding=4)
logo = ui.image(root, "art/logo.png")
fitted = ui.image(root, "art/logo.png", width=60, height=60, fit="contain")
sheet = ui.image(root, "art/sheet.png", cols=4, rows=2, frame=5)
card = ui.panel(root, width=40, height=20, fill=0x204060ff, radius=4, shadow=(3, 1, 0x000000a0),
                gradient=(0x102030ff, "down"), fade=0.25)
slice_bg = ui.panel(root, width=30, height=30, slice=("art/frame.png", (3, 3, 3, 3)))
hidden = ui.panel(root, width=50, height=50, display=False)
after = ui.panel(root, width=10, height=10)
btn = ui.button(root, "Go", fill=0x404040ff, transition=4,
                states={"hover": {"fill": 0xe8741aff}, "disabled": {"fill": 0x222222ff}})

def rect_of(e):
    r = e.get(Rect)
    return (r.x, r.y, r.w, r.h)
def shown_fill():
    return btn.get(Shown).fill if btn.has(Shown) else None
"#;

fn styled() -> Ui {
    let world = World::new();
    let ecs = Ecs::new(&world, 3, "game").unwrap();
    sage_ui::setup(&world, &ecs).unwrap();
    for (path, size) in [("art/logo.png", [32, 16]), ("art/sheet.png", [64, 32])] {
        sage_ui::register_image(&ecs, sage_assets::AssetId::of_path(path).0, size);
    }
    let kit = sage_ui::load_kit(&ecs).unwrap();
    let script = ecs.load("game", STYLED).unwrap();
    Ui {
        ecs,
        script,
        _kit: kit,
        world,
    }
}

#[test]
fn the_kit_hashes_asset_paths_like_the_asset_store_does() {
    let ui = styled();
    for path in [
        "art/logo.png",
        "./art/logo.png",
        "art\\logo.png",
        "a.png",
        "ü/β.png",
    ] {
        let want = sage_assets::AssetId::of_path(path).0;
        assert_eq!(
            ui.eval::<u64>(&format!("ui.asset({path:?})")),
            want,
            "{path}"
        );
    }
}

#[test]
fn images_size_themselves_and_fit_inside_boxes() {
    let ui = styled();
    ui.tick();
    let r = |e: &str| ui.eval::<(f32, f32, f32, f32)>(&format!("rect_of({e})"));
    assert_eq!(
        (r("logo").2, r("logo").3),
        (32.0, 16.0),
        "an auto-sized image is its natural size"
    );
    assert_eq!(
        (r("sheet").2, r("sheet").3),
        (16.0, 16.0),
        "a sheet's size is one frame"
    );
    assert_eq!((r("fitted").2, r("fitted").3), (60.0, 60.0));
    let f = sage_ui::frame(&ui.world, &ui.ecs);
    let images: Vec<_> = f
        .draws
        .iter()
        .filter_map(|d| match d {
            UiDraw::Image { pos, size, uv, .. } => Some((*pos, *size, *uv)),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 3);
    // Contain: a 32x16 image in a 60x60 box is 60x30, centered vertically.
    let (pos, size, _) = images[1];
    assert_eq!((size, pos[1] - r("fitted").1), ([60.0, 30.0], 15.0));
    // Frame 5 of a 4x2 sheet: column 1, row 1.
    assert_eq!(images[2].2, [0.25, 0.5, 0.5, 1.0]);
}

#[test]
fn decor_slices_and_display_none_reach_the_draw_list() {
    let ui = styled();
    ui.tick();
    let f = sage_ui::frame(&ui.world, &ui.ecs);
    assert!(
        f.draws
            .iter()
            .any(|d| matches!(d, UiDraw::Shadow { blur, .. } if *blur == 3.0))
    );
    assert!(
        f.draws
            .iter()
            .any(|d| matches!(d, UiDraw::Gradient { colors, .. }
            if colors[0][3] == 0xbf && colors[2][3] == 0xbf)),
        "a faded gradient has 75% alpha: {:?}",
        f.draws
    );
    assert!(
        f.draws
            .iter()
            .any(|d| matches!(d, UiDraw::Slice { insets, .. } if *insets == [3.0; 4]))
    );
    // The hidden panel takes no space: `after` sits right below the slice panel.
    let r = |e: &str| ui.eval::<(f32, f32, f32, f32)>(&format!("rect_of({e})"));
    assert_eq!(r("hidden"), (0.0, 0.0, 0.0, 0.0));
    assert_eq!(r("after").1, r("slice_bg").1 + 30.0 + 4.0);
    // Showing it pushes the next one down.
    Python::attach(|py| {
        py.run(c"hidden.set(Display(0))", Some(ui.script.globals(py)), None)
            .unwrap();
    });
    ui.tick();
    assert_eq!(r("after").1, r("slice_bg").1 + 30.0 + 4.0 + 50.0 + 4.0);
}

#[test]
fn state_variants_and_transitions_ease_in_whole_ticks() {
    let ui = styled();
    ui.tick();
    ui.tick();
    assert_eq!(
        ui.eval::<u32>("shown_fill()"),
        0x4040_40ff,
        "starts at its resting fill"
    );
    // Hover the button: the fill eases to the hover variant (time constant 4).
    let (x, y) = ui.eval::<(f32, f32)>("(lambda r: (r.x + 2, r.y + 2))(btn.get(Rect))");
    let mut seen = vec![];
    for _ in 0..60 {
        ui.pointer(x, y, false);
        ui.tick();
        seen.push(ui.eval::<u32>("shown_fill()"));
    }
    assert_ne!(seen[0], 0xe874_1aff, "not there yet after one tick");
    assert_eq!(*seen.last().unwrap(), 0xe874_1aff, "arrived: {seen:x?}");
    let red = |c: u32| (c >> 24) as i32;
    assert!(
        seen.windows(2).all(|w| red(w[1]) >= red(w[0])),
        "the red channel rises monotonically: {seen:x?}"
    );
    // The frame draws the shown color, not the jump.
    let f = sage_ui::frame(&ui.world, &ui.ecs);
    assert!(
        f.draws
            .iter()
            .any(|d| matches!(d, UiDraw::Rect { color, .. } if *color == [0xe8, 0x74, 0x1a, 0xff]))
    );
    // It is state: it hashes and a save game restores it mid-transition.
    ui.no_pointer();
    ui.tick();
    let text = ui.ecs.save_game(Default::default());
    let at_save = ui.hash();
    let later: Vec<u64> = (0..6)
        .map(|_| {
            ui.tick();
            ui.hash()
        })
        .collect();
    let other = styled();
    other.tick();
    other.ecs.load_game(&text, Policy::Strict).unwrap();
    assert_eq!(other.hash(), at_save);
    for (i, want) in later.iter().enumerate() {
        other.no_pointer();
        other.tick();
        assert_eq!(other.hash(), *want, "tick {} after loading", i + 1);
    }
}
