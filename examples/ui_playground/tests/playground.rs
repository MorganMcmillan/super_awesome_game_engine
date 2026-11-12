use sage_core::StateHasher;
use sage_core::host::{
    NAV_ACCEPT, NAV_BACKSPACE, NAV_CANCEL, NAV_COPY, NAV_CUT, NAV_END, NAV_HOME, NAV_LEFT,
    NAV_NEXT, NAV_PAGE_DOWN, NAV_PASTE, NAV_REDO, NAV_SELECT_ALL, NAV_SHIFT, NAV_UNDO, NAV_UP,
    NAV_WORD, UiDraw,
};
use sage_ecs_py::Policy;
use sage_platform::ui_window::UiTickInput;
use ui_playground::{Bouncer, Playground, SCREEN};

const SCALE: u32 = 2;
const SIZE: [u32; 2] = [SCREEN[0] as u32 * SCALE, SCREEN[1] as u32 * SCALE];

fn pixel(img: &sage_assets::Image, ui: [f32; 2]) -> [u8; 3] {
    let p = img.pixel((ui[0] * SCALE as f32) as u32, (ui[1] * SCALE as f32) as u32);
    [p[0], p[1], p[2]]
}

fn luma(p: [u8; 3]) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

fn hash(p: &Playground) -> u64 {
    let mut h = StateHasher::new();
    p.ecs.hash_into(&mut h);
    h.finish()
}

/// The focused entity's StableId (0 for none), read from the interaction
/// singleton.
fn focus(p: &Playground) -> u64 {
    let id = p.ecs.lookup_path("sage.ui.Interaction");
    p.ecs.field_u64(id, id, "focus").unwrap()
}

fn texts(p: &Playground) -> Vec<String> {
    p.ui_frame()
        .draws
        .into_iter()
        .filter_map(|d| match d {
            UiDraw::Text { text, .. } => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn the_palette_is_orange_on_dark_backgrounds() {
    let p = Playground::new().unwrap();
    p.idle(2);
    let img = p.render(SIZE);
    // The app background and a card are dark; the primary button is orange.
    let app = pixel(&img, [1.0, 1.0]);
    let [cx, cy, cw, _] = p.rect("buttons");
    let card = pixel(&img, [cx + cw - 2.0, cy + 2.0]);
    let [px, py, _, ph] = p.rect("primary");
    let primary = pixel(&img, [px + 2.0, py + ph - 2.0]);
    assert_eq!(app, [0x0F, 0x0D, 0x0B], "the app is the darkest surface");
    assert_eq!(card, [0x1B, 0x17, 0x14], "cards are one step lighter");
    assert_eq!(primary, [0xF2, 0x8C, 0x28], "the primary color is orange");
    assert!(luma(app) < luma(card) && luma(card) < 40.0);
    // Most of the screen is a dark background; orange is an accent.
    let (mut dark, mut orange, total) = (0usize, 0usize, (SIZE[0] * SIZE[1]) as usize);
    for y in 0..SIZE[1] {
        for x in 0..SIZE[0] {
            let p = img.pixel(x, y);
            if luma([p[0], p[1], p[2]]) < 48.0 {
                dark += 1;
            }
            if p[0] > 220 && (120..160).contains(&p[1]) && p[2] < 70 {
                orange += 1;
            }
        }
    }
    assert!(
        dark * 100 / total > 60,
        "dark share {}%",
        dark * 100 / total
    );
    assert!(
        orange > 0 && orange * 100 / total < 25,
        "orange share {}%",
        orange * 100 / total
    );
}

#[test]
fn every_widget_fits_on_the_screen() {
    let p = Playground::new().unwrap();
    let vars = [
        "bar",
        "buttons",
        "toggles",
        "keys",
        "sliders",
        "entry",
        "themes",
        "styles",
        "layout",
        "scrolling",
        "primary",
        "second",
        "ghost",
        "danger",
        "disabled",
        "clicks",
        "sound",
        "music",
        "extras",
        "extra_button",
        "volume",
        "brightness",
        "progress",
        "name",
        "greeting",
        "scroll_area",
        "dialog_button",
    ];
    for var in vars {
        let [x, y, w, h] = p.rect(var);
        assert!(
            x >= 0.0 && y >= 0.0 && x + w <= SCREEN[0] && y + h <= SCREEN[1],
            "{var} is at {:?}, outside the {SCREEN:?} screen",
            [x, y, w, h]
        );
        assert!(w > 0.0 && h > 0.0, "{var} has no size");
    }
}

#[test]
fn buttons_count_clicks_and_a_disabled_button_does_nothing() {
    let p = Playground::new().unwrap();
    assert_eq!(p.text("clicks"), "CLICKS: 0");
    for _ in 0..3 {
        p.click("primary");
    }
    assert_eq!(p.text("clicks"), "CLICKS: 3");
    p.click("second");
    assert_eq!(p.text("clicks"), "CLICKS: 3");
    p.click("disabled");
    assert_eq!(focus(&p), 0, "a disabled button cannot take focus");
    assert_eq!(p.text("clicks"), "CLICKS: 3");
}

#[test]
fn a_toggle_enables_and_disables_another_widget() {
    let p = Playground::new().unwrap();
    assert_eq!(p.eval::<u32>("extra_button.get(Widget).enabled"), 1);
    p.click("extras");
    assert_eq!(p.eval::<u32>("extras.get(Toggle).on"), 0);
    assert_eq!(p.eval::<u32>("extra_button.get(Widget).enabled"), 0);
    p.click("extras");
    assert_eq!(p.eval::<u32>("extra_button.get(Widget).enabled"), 1);
}

#[test]
fn sliders_report_values_and_the_meter_animates_by_itself() {
    let p = Playground::new().unwrap();
    let [x, y, w, h] = p.rect("volume");
    p.click_at([x + w * 0.3, y + h / 2.0]);
    assert_eq!(p.eval::<f32>("volume.get(Slider).value"), 3.0);
    assert_eq!(p.text("volume_label"), "3");
    let [x, y, w, h] = p.rect("brightness");
    // The right edge itself is outside the widget; one unit in is the maximum.
    p.click_at([x + w - 1.0, y + h / 2.0]);
    assert_eq!(p.text("bright_label"), "5 OF 5");
    // The progress bar is a system over a tag: it fills with no input at all.
    let before = p.eval::<f32>("progress.get(Slider).value");
    p.idle(60);
    let after = p.eval::<f32>("progress.get(Slider).value");
    assert!(after > before + 0.3 && after <= 1.0, "{before} -> {after}");
    // And a meter ignores the pointer.
    p.click("progress");
    assert_eq!(focus(&p), 0);
}

#[test]
fn typing_in_the_name_field_updates_the_greeting_at_the_caret() {
    let p = Playground::new().unwrap();
    assert_eq!(p.text("greeting"), "HELLO, ADA!");
    let [x, y, w, h] = p.rect("name");
    p.click_at([x + w - 2.0, y + h / 2.0]);
    assert_eq!(
        p.eval::<u32>("name.get(Caret).pos"),
        3,
        "a click past the end puts the caret there"
    );
    p.type_text("m");
    assert_eq!(p.text("greeting"), "HELLO, ADAM!");
    p.press(sage_core::host::NAV_HOME);
    p.type_text(">");
    assert_eq!(p.text("name"), ">Adam");
    assert_eq!(p.text("greeting"), "HELLO, >ADAM!");
    p.press(NAV_END);
    assert_eq!(p.eval::<u32>("name.get(Caret).pos"), 5);
}

#[test]
fn a_theme_is_one_edit_of_the_prefabs_and_every_instance_follows() {
    let p = Playground::new().unwrap();
    let orange = [0xF2, 0x8C, 0x28];
    let teal = [0x2E, 0xC4, 0xB6];
    let at = |p: &Playground, var: &str| {
        let [x, y, _, h] = p.rect(var);
        pixel(&p.render(SIZE), [x + 2.0, y + h - 2.0])
    };
    assert_eq!(at(&p, "primary"), orange);
    assert_eq!(at(&p, "dialog_button"), orange);
    let swatch_violet = at(&p, "swatch_violet");
    // Click the TEAL swatch.
    p.click("swatch_teal");
    assert_eq!(at(&p, "primary"), teal, "an instance of the Primary prefab");
    assert_eq!(at(&p, "dialog_button"), teal, "another instance of it");
    // Different prefabs follow too: text, switches, the meter.
    let img = p.render(SIZE);
    let [x, y, w, h] = p.rect("volume");
    assert_eq!(
        pixel(&img, [x + 2.0, y + h / 2.0]),
        teal,
        "the slider's fill"
    );
    let [x, y, _, _] = p.rect("ghost");
    assert_eq!(
        pixel(&img, [x + 0.5, y + 3.0]),
        teal,
        "the ghost button's border"
    );
    assert!(w > 0.0);
    // The swatches keep their own fills: they set their own Paint.
    assert_eq!(at(&p, "swatch_violet"), swatch_violet);
    // And back.
    p.click("swatch_orange");
    assert_eq!(at(&p, "primary"), orange);
    // No widget overrides the group: all of them read the prefab.
    assert_eq!(p.eval::<u32>("primary.get(Paint).fill"), 0xF28C_28FF);
}

#[test]
fn the_dialog_is_a_modal() {
    let p = Playground::new().unwrap();
    assert!(!texts(&p).contains(&"DIALOG".to_string()));
    let before = texts(&p);
    p.click("dialog_button");
    assert!(
        texts(&p).contains(&"DIALOG".to_string()),
        "the dialog opened"
    );
    assert!(p.eval::<bool>("ui.is_open(dialog_back)"));
    // The backdrop blocks the pointer: a click on a button behind it is lost.
    p.click("primary");
    assert_eq!(p.text("clicks"), "CLICKS: 0");
    // Keyboard focus stays inside: tabbing visits only the dialog's two buttons.
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..6 {
        p.press(NAV_NEXT);
        seen.insert(focus(&p));
    }
    assert_eq!(seen.len(), 2, "{seen:?}");
    let both: Vec<u64> = (0..2)
        .map(|k| p.eval::<u64>(&format!("dialog_buttons()[{k}].sid")))
        .collect();
    assert!(
        seen.iter().all(|s| both.contains(s)),
        "{seen:?} vs {both:?}"
    );
    // OK closes it, and the tree is back to what it was.
    let ok = both[1];
    let target = {
        let e = p.ecs.entity_of_sid(ok).unwrap();
        let id = p.ecs.lookup_path("sage.ui.Rect");
        let r = ["x", "y", "w", "h"].map(|f| p.ecs.field(e, id, f).unwrap() as f32);
        [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
    };
    p.click_at(target);
    assert!(
        !texts(&p).contains(&"DIALOG".to_string()),
        "the dialog closed"
    );
    assert!(!p.eval::<bool>("ui.is_open(dialog_back)"));
    p.idle(2);
    assert_eq!(texts(&p), before, "the same text is on screen as before");
    p.click("primary");
    assert_eq!(
        p.text("clicks"),
        "CLICKS: 1",
        "the pointer reaches the buttons again"
    );
}

#[test]
fn the_list_scrolls_with_the_wheel_the_keyboard_and_focus() {
    let p = Playground::new().unwrap();
    let c = p.center("scroll_area");
    assert_eq!(p.eval::<f32>("scroll_area.get(Scroll).offset"), 0.0);
    p.wheel_at(c, 2.0);
    let after_wheel = p.eval::<f32>("scroll_area.get(Scroll).offset");
    assert_eq!(after_wheel, 24.0);
    p.hover(c);
    p.press(NAV_PAGE_DOWN);
    assert!(p.eval::<f32>("scroll_area.get(Scroll).offset") > after_wheel);
    // Tabbing to the last item scrolls it into view.
    for _ in 0..40 {
        p.press(NAV_NEXT);
        if p.eval::<bool>(&format!("items[-1].sid == {}", focus(&p))) {
            break;
        }
    }
    let [_, sy, _, sh] = p.rect("scroll_area");
    let [_, iy, _, ih] = p.rect("items[-1]");
    assert!(
        iy >= sy && iy + ih <= sy + sh,
        "the focused item is inside the list's view"
    );
}

#[test]
fn the_playground_is_deterministic_and_survives_a_save_game() {
    let session = |p: &Playground| {
        p.click("primary");
        p.type_text("x");
        let [x, y, w, h] = p.rect("volume");
        p.click_at([x + w * 0.6, y + h / 2.0]);
        p.click("swatch_violet");
        p.click("dialog_button");
        p.idle(5);
    };
    let (a, b) = (Playground::new().unwrap(), Playground::new().unwrap());
    session(&a);
    session(&b);
    assert_eq!(hash(&a), hash(&b), "the same inputs give the same world");
    assert_eq!(
        a.render(SIZE).to_png_bytes().unwrap(),
        b.render(SIZE).to_png_bytes().unwrap(),
        "and the same picture"
    );
    // Save with the dialog open and the theme changed, load into a fresh one.
    let text = a.ecs.save_game(Default::default());
    let c = Playground::new().unwrap();
    c.idle(9);
    c.ecs.load_game(&text, Policy::Strict).unwrap();
    assert_eq!(hash(&c), hash(&a));
    for _ in 0..30 {
        a.idle(1);
        c.idle(1);
        assert_eq!(hash(&c), hash(&a));
    }
    assert_eq!(
        c.render(SIZE).to_png_bytes().unwrap(),
        a.render(SIZE).to_png_bytes().unwrap()
    );
    // The loaded dialog still works: OK closes it.
    assert!(c.eval::<bool>("ui.is_open(dialog_back)"));
    let _ = (Policy::Lenient, NAV_END);
}

// ---- the STYLE page ------------------------------------------------------------

fn draws(p: &Playground) -> Vec<UiDraw> {
    p.ui_frame().draws
}

#[test]
fn the_style_page_is_hidden_until_its_tab_is_chosen() {
    let p = Playground::new().unwrap();
    assert_eq!(
        p.rect("style_root"),
        [0.0; 4],
        "a hidden page takes no space"
    );
    assert!(!texts(&p).contains(&"IMAGES".to_string()));
    assert!(texts(&p).contains(&"BUTTONS".to_string()));
    p.show_page("STYLE");
    assert!(texts(&p).contains(&"IMAGES".to_string()));
    assert!(
        !texts(&p).contains(&"BUTTONS".to_string()),
        "the first page is hidden now"
    );
    assert_eq!(p.eval::<u32>("tab_buttons[1].get(Toggle).on"), 1);
    assert_eq!(p.eval::<u32>("tab_buttons[0].get(Toggle).on"), 0);
    // Clicking the selected tab keeps it selected.
    p.click("tab_buttons[1]");
    assert_eq!(p.eval::<u32>("tab_buttons[1].get(Toggle).on"), 1);
    // And back.
    p.show_page("BASICS");
    assert!(texts(&p).contains(&"BUTTONS".to_string()));
    assert_eq!(p.rect("style_root"), [0.0; 4]);
}

#[test]
fn images_render_with_their_pixels_not_the_missing_asset_magenta() {
    let p = Playground::new().unwrap();
    p.show_page("STYLE");
    let img = p.render(SIZE);
    let [x, y, w, h] = p.rect("logo");
    assert_eq!((w, h), (32.0, 32.0), "the logo is its natural size");
    // The diamond's center is orange-red, its corner is transparent over a card.
    let center = pixel(&img, [x + w / 2.0, y + h / 2.0]);
    assert!(
        center[0] > 150 && center[1] < 150 && center[2] < 80,
        "{center:?}"
    );
    let corner = pixel(&img, [x + 1.0, y + 1.0]);
    assert_eq!(
        corner,
        [0x1B, 0x17, 0x14],
        "transparent pixels show the card"
    );
    let mut magenta = 0;
    for y in 0..SIZE[1] {
        for x in 0..SIZE[0] {
            let p = img.pixel(x, y);
            if p[0] > 240 && p[1] < 10 && p[2] > 240 {
                magenta += 1;
            }
        }
    }
    assert_eq!(magenta, 0, "no image is missing");
}

#[test]
fn fit_modes_place_the_logo_in_its_boxes() {
    let p = Playground::new().unwrap();
    p.show_page("STYLE");
    type Placed = ([f32; 2], [f32; 2], Option<[f32; 4]>);
    let images: Vec<Placed> = draws(&p)
        .into_iter()
        .filter_map(|d| match d {
            UiDraw::Image {
                pos,
                size,
                clip,
                asset,
                ..
            } if asset == sage_assets::AssetId::of_path("art/logo.png").0 => {
                Some((pos, size, clip))
            }
            _ => None,
        })
        .collect();
    // The natural one, then contain, cover and none in 40 x 28 boxes.
    assert_eq!(images.len(), 4);
    assert_eq!(images[1].1, [28.0, 28.0], "contain");
    assert_eq!(images[2].1, [40.0, 40.0], "cover");
    assert_eq!(images[3].1, [32.0, 32.0], "none keeps the natural size");
    let clip = images[2].2.expect("cover is cut at its box");
    assert_eq!((clip[2] - clip[0], clip[3] - clip[1]), (40.0, 28.0));
}

#[test]
fn the_sprite_sheet_player_advances_a_system_driven_frame() {
    let p = Playground::new().unwrap();
    p.show_page("STYLE");
    let frame = |p: &Playground| p.eval::<u32>("spinner.get(Image).frame");
    let first = frame(&p);
    p.idle(8);
    assert_eq!(frame(&p), (first + 1) % 4, "one frame every 8 ticks");
    p.idle(8 * 4);
    assert_eq!(frame(&p), (first + 1) % 4, "and it wraps after four");
    // The frame the draw list shows is the frame in the component.
    let uv = draws(&p)
        .into_iter()
        .find_map(|d| match d {
            UiDraw::Image { uv, asset, .. }
                if asset == sage_assets::AssetId::of_path("art/spinner.png").0 =>
            {
                Some(uv)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(uv[0], frame(&p) as f32 / 4.0);
}

#[test]
fn boxes_have_rounded_corners_shadows_gradients_and_a_slice_frame() {
    let p = Playground::new().unwrap();
    p.show_page("STYLE");
    let d = draws(&p);
    assert!(
        d.iter()
            .any(|d| matches!(d, UiDraw::Round { radius, .. } if *radius == 6.0)),
        "rounded"
    );
    assert!(
        d.iter()
            .any(|d| matches!(d, UiDraw::Shadow { blur, .. } if *blur == 5.0)),
        "shadow"
    );
    assert!(
        d.iter().any(|d| matches!(d, UiDraw::Gradient { .. })),
        "gradient"
    );
    assert!(
        d.iter()
            .any(|d| matches!(d, UiDraw::Slice { asset, insets, .. }
            if *asset == sage_assets::AssetId::of_path("art/frame.png").0 && *insets == [4.0; 4])),
        "slice"
    );
    // The gradient really runs orange to teal in the picture.
    let img = p.render(SIZE);
    let [x, y, w, h] = p.rect("gradient");
    let left = pixel(&img, [x + 2.0, y + h - 2.0]);
    let right = pixel(&img, [x + w - 3.0, y + h - 2.0]);
    assert!(
        left[0] > 200 && left[2] < 120,
        "orange at the left: {left:?}"
    );
    assert!(
        right[1] > 150 && right[0] < 120,
        "teal at the right: {right:?}"
    );
}

#[test]
fn the_fade_slider_fades_a_group_and_its_children() {
    let p = Playground::new().unwrap();
    p.show_page("STYLE");
    let btn_alpha = |p: &Playground| {
        draws(p)
            .into_iter()
            .find_map(|d| match d {
                UiDraw::Text { text, color, .. } if text == "CHILDREN FADE TOO" => Some(color[3]),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(btn_alpha(&p), 255);
    let [x, y, w, h] = p.rect("fade_slider");
    p.click_at([x + w * 0.5, y + h / 2.0]); // 5 of 10: half faded
    assert_eq!(p.eval::<f32>("fade_group.get(Decor).fade"), 0.5);
    let half = btn_alpha(&p);
    assert!((126..=129).contains(&half), "{half}");
    p.click_at([x + w - 1.0, y + h / 2.0]); // fully faded
    assert_eq!(btn_alpha(&p), 0);
}

#[test]
fn display_none_hides_and_shows_a_note_and_the_layout_follows() {
    let p = Playground::new().unwrap();
    p.show_page("STYLE");
    assert_eq!(p.rect("note"), [0.0; 4], "hidden: no space");
    let below_before = p.rect("fading")[3];
    p.click("extra_box");
    p.idle(2);
    let [_, _, w, h] = p.rect("note");
    assert!(w > 0.0 && h > 0.0, "shown");
    assert!(
        p.rect("fading")[3] > below_before,
        "the card grew to hold it"
    );
    assert!(texts(&p).iter().any(|t| t.contains("THIS NOTE")));
    p.click("extra_box");
    p.idle(2);
    assert_eq!(p.rect("note"), [0.0; 4]);
    assert!(!texts(&p).iter().any(|t| t.contains("THIS NOTE")));
}

#[test]
fn hover_transitions_differ_by_time_constant_and_a_disabled_button_keeps_its_variant() {
    let p = Playground::new().unwrap();
    p.show_page("STYLE");
    let fill = |p: &Playground, var: &str| p.eval::<u32>(&format!("{var}.get(Shown).fill"));
    let hover = |name: &str| {
        let c = p.center(&format!("speeds[{name:?}]"));
        p.hover(c);
    };
    let before = fill(&p, "speeds['SLOW 16']");
    // After two ticks of hovering, the fast one is much nearer its target.
    hover("SLOW 16");
    p.hover(p.center("speeds['FAST 2']"));
    p.hover(p.center("speeds['FAST 2']"));
    let slow = fill(&p, "speeds['SLOW 16']");
    let fast = fill(&p, "speeds['FAST 2']");
    let target = 0xF28C_28FFu32;
    let dist = |c: u32| (c >> 24).abs_diff(target >> 24);
    assert!(
        dist(fast) < dist(slow),
        "fast {fast:x} vs slow {slow:x} (rest {before:x})"
    );
    // INSTANT jumps in one tick.
    p.hover(p.center("speeds['INSTANT']"));
    assert_eq!(fill(&p, "speeds['INSTANT']"), target);
    // The disabled button is not hoverable and uses its own variant.
    p.hover(p.center("disabled_state"));
    assert_eq!(fill_of(&p, "disabled_state"), [0x1B, 0x17, 0x14, 0xFF]);
    // A checked choice uses its checked variant, and unchecked ones do not.
    let (a, b) = (fill_of(&p, "choice_a"), fill_of(&p, "choice_b"));
    assert_eq!(a, [0xF2, 0x8C, 0x28, 0xFF]);
    assert_ne!(b, [0x2E, 0xC4, 0xB6, 0xFF]);
    p.click("choice_b");
    p.hover([1.0, 1.0]); // away from it: hover would lighten the fill
    assert_eq!(fill_of(&p, "choice_b"), [0x2E, 0xC4, 0xB6, 0xFF]);
}

fn fill_of(p: &Playground, var: &str) -> [u8; 4] {
    let [x, y, _, _] = p.rect(var);
    let img = p.render(SIZE);
    let q = img.pixel((x * SCALE as f32) as u32 + 5, (y * SCALE as f32) as u32 + 5);
    [q[0], q[1], q[2], q[3]]
}

#[test]
fn the_style_page_survives_a_save_game_mid_transition() {
    let session = |p: &Playground| {
        p.show_page("STYLE");
        p.hover(p.center("speeds['SLOW 16']"));
        p.idle(3);
    };
    let (a, b) = (Playground::new().unwrap(), Playground::new().unwrap());
    session(&a);
    session(&b);
    assert_eq!(hash(&a), hash(&b));
    let text = a.ecs.save_game(Default::default());
    let c = Playground::new().unwrap();
    c.ecs.load_game(&text, Policy::Strict).unwrap();
    assert_eq!(hash(&c), hash(&a));
    for _ in 0..30 {
        a.idle(1);
        c.idle(1);
        assert_eq!(hash(&c), hash(&a));
    }
    assert_eq!(
        c.render(SIZE).to_png_bytes().unwrap(),
        a.render(SIZE).to_png_bytes().unwrap()
    );
}

// -- the popups page ------------------------------------------------------------

fn popups() -> Playground {
    let p = Playground::new().unwrap();
    p.show_page("POPUPS");
    p
}

#[test]
fn a_select_opens_a_list_picks_an_option_and_closes() {
    let p = popups();
    let list = "world.lookup(color_select.get(Select).popup)";
    assert_eq!(p.rect(list), [0.0; 4], "closed, the list takes no space");
    p.click("color_select");
    p.idle(2);
    assert!(
        texts(&p).contains(&"VIOLET".to_string()),
        "the list is open"
    );
    let [sx, sy, sw, sh] = p.rect("color_select");
    let r = p.rect(list);
    assert!(r[1] >= sy + sh - 1.0, "below the select: {r:?}");
    assert!(
        (r[0] - sx).abs() < 1.5 && (r[2] - sw).abs() < 1.5,
        "as wide: {r:?}"
    );
    p.click("popup_option(color_select, 2)");
    p.idle(2);
    assert_eq!(p.text("color_said"), "CHOSEN: VIOLET");
    assert!(!texts(&p).contains(&"TEAL".to_string()), "the list closed");
}

#[test]
fn a_combo_box_narrows_its_list_as_you_type_and_picks_one() {
    let p = popups();
    p.click("fruit_combo");
    p.type_text("AP");
    p.idle(2);
    let t = texts(&p);
    assert!(t.contains(&"APPLE".to_string()) && t.contains(&"APRICOT".to_string()));
    assert!(
        !t.contains(&"BANANA".to_string()),
        "non-matches are hidden: {t:?}"
    );
    p.click("popup_option(fruit_combo, 1)");
    p.idle(2);
    assert_eq!(p.text("fruit_said"), "PICKED: APRICOT");
    assert!(!texts(&p).contains(&"AVOCADO".to_string()), "closed");
}

#[test]
fn the_menu_bar_opens_submenus_and_chooses_items() {
    let p = popups();
    p.click("file_menu");
    p.idle(2);
    assert!(
        texts(&p).contains(&"RECENT".to_string()),
        "the file menu is open"
    );
    p.click("menu_item(file_menu, 'RECENT')");
    p.idle(2);
    assert!(
        texts(&p).contains(&"TWO.TXT".to_string()),
        "the submenu is open"
    );
    p.click("menu_item(file_menu, 'RECENT', 'TWO.TXT')");
    p.idle(2);
    assert_eq!(p.text("menu_said"), "CHOSEN: TWO.TXT");
    assert!(
        !texts(&p).contains(&"RECENT".to_string()),
        "choosing closes every menu"
    );
    // A disabled item does nothing.
    p.click("file_menu");
    p.idle(2);
    p.click("menu_item(file_menu, 'SAVE')");
    p.idle(2);
    assert_eq!(p.text("menu_said"), "CHOSEN: TWO.TXT");
    // Hovering another title while one is open switches menus.
    p.hover(p.center("view_menu"));
    p.idle(2);
    assert!(texts(&p).contains(&"RULERS".to_string()));
    assert!(!texts(&p).contains(&"RECENT".to_string()));
    // A check item toggles.
    p.click("menu_item(view_menu, 'RULERS')");
    p.idle(2);
    assert_eq!(p.text("menu_said"), "CHOSEN: RULERS");
    // Escape closes.
    p.click("view_menu");
    p.idle(2);
    p.press(sage_core::host::NAV_CANCEL);
    p.idle(2);
    assert!(!texts(&p).contains(&"RULERS".to_string()));
}

#[test]
fn a_right_click_opens_the_context_menu_at_the_pointer() {
    let p = popups();
    let [x, y, w, h] = p.rect("context_area");
    let at = [x + w / 4.0, y + h / 2.0];
    p.input(&sage_platform::ui_window::UiTickInput {
        pointer: Some(at),
        secondary: true,
        ..Default::default()
    });
    p.input(&sage_platform::ui_window::UiTickInput {
        pointer: Some(at),
        ..Default::default()
    });
    p.idle(2);
    assert!(texts(&p).contains(&"PASTE".to_string()), "open");
    let first = p.rect("menu_item(context_popup, 'COPY')");
    assert!(
        (first[0] - at[0]).abs() < 12.0,
        "next to the pointer: {first:?} {at:?}"
    );
    p.click("menu_item(context_popup, 'DELETE')");
    p.idle(2);
    assert_eq!(p.text("context_said"), "LAST: DELETE");
    assert!(!texts(&p).contains(&"PASTE".to_string()));
}

#[test]
fn a_tooltip_appears_after_its_delay_and_goes_when_the_pointer_leaves() {
    let p = popups();
    let rest = |at: [f32; 2], ticks: u32| (0..ticks).for_each(|_| p.hover(at));
    let shown = |prefix: &str| texts(&p).iter().any(|t| t.starts_with(prefix));
    let c = p.center("tip_button");
    rest(c, 5);
    assert!(!shown("A TOOLTIP APPEARS"), "too early");
    rest(c, 20);
    assert!(shown("A TOOLTIP APPEARS"), "after the delay");
    rest([2.0, 2.0], 2);
    assert!(!shown("A TOOLTIP APPEARS"), "gone");
    // The slower one is not up at 30 ticks.
    let c = p.center("slow_button");
    rest(c, 30);
    assert!(!shown("THIS ONE WAITS"));
    rest(c, 50);
    assert!(shown("THIS ONE WAITS"));
    // A tooltip on a group shows for hovering its child.
    rest(p.center("tip_inner"), 25);
    assert!(shown("A TOOLTIP ON A GROUP"));
    // Pressing hides it.
    p.click("tip_inner");
    assert!(!shown("A TOOLTIP ON A GROUP"));
}

#[test]
fn the_confirm_dialog_returns_focus_to_its_button_and_escape_cancels() {
    let p = popups();
    p.click("confirm_button");
    p.idle(2);
    assert!(p.eval::<bool>("ui.is_open(confirm_back)"));
    p.press(sage_core::host::NAV_CANCEL);
    p.idle(2);
    assert!(!p.eval::<bool>("ui.is_open(confirm_back)"));
    assert_eq!(
        focus(&p),
        p.eval::<u64>("confirm_button.sid"),
        "focus returned"
    );
    p.click("confirm_button");
    p.idle(2);
    p.click("confirm_yes");
    p.idle(2);
    assert_eq!(p.text("confirm_said"), "ANSWER: YES");
    assert!(!p.eval::<bool>("ui.is_open(confirm_back)"));
}

// -- the text page --------------------------------------------------------------

fn texty() -> Playground {
    let p = Playground::new().unwrap();
    p.show_page("TEXT");
    p
}

fn hold(p: &Playground, bits: u32) {
    p.press(bits);
}

#[test]
fn a_field_shows_its_placeholder_until_typed_in_and_counts_what_is_typed() {
    let p = texty();
    assert!(texts(&p).contains(&"NAME OR EMAIL".to_string()));
    p.click("email_field");
    p.type_text("ada");
    assert!(!texts(&p).contains(&"NAME OR EMAIL".to_string()));
    assert_eq!(p.text("email_said"), "LENGTH: 3");
    // The limit is 40 characters.
    p.type_text(&"x".repeat(60));
    assert_eq!(p.text("email_said"), "LENGTH: 40");
}

#[test]
fn selection_replaces_and_the_clipboard_round_trips_to_the_window() {
    let p = texty();
    p.click("email_field");
    p.type_text("hello world");
    hold(&p, NAV_LEFT | NAV_WORD | NAV_SHIFT);
    hold(&p, NAV_COPY);
    assert_eq!(p.clipboard(), "world");
    hold(&p, NAV_CUT);
    assert_eq!(p.text("email_field"), "hello ");
    hold(&p, NAV_HOME);
    p.input(&UiTickInput {
        nav: NAV_PASTE,
        paste: Some("big ".into()),
        ..UiTickInput::default()
    });
    p.idle(1);
    assert_eq!(p.text("email_field"), "big hello ");
    // Undo twice: the paste, then the cut.
    hold(&p, NAV_UNDO);
    hold(&p, NAV_UNDO);
    assert_eq!(p.text("email_field"), "hello world");
    hold(&p, NAV_REDO);
    assert_eq!(p.text("email_field"), "hello ");
    // Select all and type: it replaces everything.
    hold(&p, NAV_SELECT_ALL);
    p.type_text("Z");
    assert_eq!(p.text("email_field"), "Z");
}

#[test]
fn dragging_the_mouse_selects_text() {
    let p = texty();
    p.click("short_field");
    hold(&p, NAV_HOME);
    let [x, y, _, h] = p.rect("short_field");
    let (left, row) = (x + 3.0, y + h / 2.0);
    for (at, down) in [
        (left + 1.0, true),
        (left + 6.0 * 4.0, true),
        (left + 6.0 * 4.0, false),
    ] {
        p.input(&UiTickInput {
            pointer: Some([at, row]),
            down,
            ..UiTickInput::default()
        });
    }
    hold(&p, NAV_COPY);
    assert_eq!(p.clipboard(), "ABCD");
}

#[test]
fn a_password_shows_asterisks_and_will_not_copy() {
    let p = texty();
    p.click("password_field");
    p.type_text("hunter2");
    assert_eq!(p.text("password_said"), "TYPED: 7");
    let t = texts(&p);
    assert!(t.contains(&"*******".to_string()), "masked: {t:?}");
    assert!(!t.contains(&"hunter2".to_string()));
    hold(&p, NAV_SELECT_ALL);
    p.input(&UiTickInput {
        nav: NAV_COPY,
        ..UiTickInput::default()
    });
    assert_ne!(p.clipboard(), "hunter2");
}

#[test]
fn a_text_area_wraps_takes_enter_and_moves_by_row() {
    let p = texty();
    let rows_of = |p: &Playground| {
        p.ui_frame()
            .draws
            .iter()
            .filter(|d| matches!(d, UiDraw::Text { text, .. } if text.len() > 6 && !text.contains(':') && text.chars().all(|c| c.is_uppercase() || c == ' ' || c == '.')))
            .count()
    };
    assert!(rows_of(&p) >= 4, "the long first line wraps into rows");
    p.click("notes_area");
    hold(&p, NAV_SELECT_ALL);
    p.type_text("a");
    hold(&p, NAV_ACCEPT);
    p.type_text("b");
    assert_eq!(p.text("notes_area"), "a\nb");
    assert_eq!(p.text("area_said"), "LINES: 2");
    hold(&p, NAV_UP);
    p.type_text("X");
    assert_eq!(
        p.text("notes_area"),
        "aX\nb",
        "Up moved to the first row, not to another widget"
    );
}

#[test]
fn number_fields_clamp_snap_step_and_report() {
    let p = texty();
    p.click("count_field");
    hold(&p, NAV_SELECT_ALL);
    p.type_text("42x");
    assert_eq!(p.text("count_field"), "42", "letters are refused");
    hold(&p, NAV_ACCEPT);
    assert_eq!(p.text("count_field"), "10", "clamped to the range");
    assert_eq!(p.text("number_said"), "COUNT 10  SPEED 1.5");
    hold(&p, NAV_UP);
    assert_eq!(p.text("count_field"), "10");
    p.click("speed_field");
    hold(&p, NAV_UP);
    assert_eq!(p.text("speed_field"), "1.75", "one step of 0.25");
    assert_eq!(p.text("number_said"), "COUNT 10  SPEED 1.75");
    // The wheel steps the focused field too, downwards for positive notches.
    p.wheel_at(p.center("speed_field"), 1.0);
    assert_eq!(p.text("speed_field"), "1.5");
    // Typing 1.6 and leaving the field snaps it to the nearest step.
    hold(&p, NAV_SELECT_ALL);
    p.type_text("1.6");
    p.click("count_field");
    assert_eq!(p.text("speed_field"), "1.5");
    assert_eq!(p.eval::<f32>("speed_field.get(Slider).value"), 1.5);
}

#[test]
fn the_text_page_survives_a_save_game_with_a_selection_and_history() {
    let p = texty();
    p.click("email_field");
    p.type_text("save me");
    hold(&p, NAV_LEFT | NAV_SHIFT);
    let h = hash(&p);
    let save = p.ecs.save_game(Default::default());
    let q = Playground::new().unwrap();
    q.ecs.load_game(&save, Policy::Strict).unwrap();
    q.idle(1);
    assert_eq!(q.text("email_field"), "save me");
    // The selection and the undo history came back: undo works in the copy.
    hold(&q, NAV_UNDO);
    assert_eq!(q.text("email_field"), "");
    let _ = h;
}

#[test]
fn rich_text_draws_styled_runs_and_a_link_click_navigates() {
    let p = texty();
    let draws = p.ui_frame().draws;
    let run = |word: &str| {
        draws.iter().find_map(|d| match d {
            UiDraw::Text {
                text, pos, color, ..
            } if text == word => Some((*pos, *color)),
            _ => None,
        })
    };
    let (_, plain) = run("A PARAGRAPH CAN BE").expect("the plain run");
    let (_, tinted) = run("TINTED").expect("a colored run");
    let (link_at, link_color) = run("LINK").expect("a link run");
    assert_eq!(tinted, [0x4F, 0xC3, 0xC0, 255]);
    assert_ne!(link_color, plain, "a link has the accent color");
    // Bold is a style on the text: one draw with the bold bit. The built-in
    // face strikes it twice when drawing; a styled face has bold glyphs.
    let bold: Vec<u8> = draws
        .iter()
        .filter_map(|d| match d {
            UiDraw::Text { text, style, .. } if text == "BOLD," || text == "BOLD" => Some(*style),
            _ => None,
        })
        .collect();
    assert_eq!(bold, [sage_core::host::STYLE_BOLD], "{bold:?}");
    // Headings are three sizes.
    let scale_of = |word: &str| {
        draws.iter().find_map(|d| match d {
            UiDraw::Text { text, scale, .. } if text == word => Some(*scale),
            _ => None,
        })
    };
    assert_eq!(
        [
            scale_of("TITLE"),
            scale_of("SECTION"),
            scale_of("SUBSECTION")
        ],
        [Some(3.0), Some(2.0), Some(1.0)]
    );
    // Clicking the link emits Navigate with its target; clicking beside it does not.
    p.click_at([link_at[0] + 6.0 * 2.0, link_at[1] + 3.0]);
    assert_eq!(p.text("link_said"), "CLICKED: PAGE:TEXT");
    p.eval::<bool>("(link_said.set(Label(world.intern('CLICKED: NOTHING'))), True)[1]");
    p.click_at([link_at[0] - 60.0, link_at[1] + 3.0]);
    assert_eq!(p.text("link_said"), "CLICKED: NOTHING");
    // A press that began beside the link and was released on it is not a
    // click on it, and neither is the other way round.
    let on = [link_at[0] + 6.0 * 2.0, link_at[1] + 3.0];
    let off = [link_at[0] - 60.0, link_at[1] + 3.0];
    for (from, to) in [(off, on), (on, off)] {
        for (pos, down) in [(from, true), (to, true), (to, false)] {
            p.input(&UiTickInput {
                pointer: Some(pos),
                down,
                ..UiTickInput::default()
            });
        }
        assert_eq!(
            p.text("link_said"),
            "CLICKED: NOTHING",
            "{from:?} to {to:?}"
        );
    }
}

#[test]
fn double_and_triple_clicks_select_a_word_and_a_line_in_a_real_field() {
    let p = texty();
    p.click("email_field");
    p.type_text("hello big world");
    let [x, y, _, h] = p.rect("email_field");
    // Characters are 6 wide with 3 of padding: "big" starts at character 6.
    let at = [x + 3.0 + 6.0 * 7.5, y + h / 2.0];
    p.click_at(at);
    p.click_at(at);
    assert_eq!(
        p.eval::<(u32, u32)>("(email_field.get(TextField).anchor, email_field.get(Caret).pos)"),
        (6, 9)
    );
    p.press(NAV_COPY);
    assert_eq!(p.clipboard(), "big");
    p.click_at(at);
    assert_eq!(
        p.eval::<(u32, u32)>("(email_field.get(TextField).anchor, email_field.get(Caret).pos)"),
        (0, 15)
    );
}

#[test]
fn an_input_methods_pre_edit_text_is_drawn_at_the_caret_until_it_is_committed() {
    let p = texty();
    p.click("email_field");
    p.type_text("ab");
    let compose = |text: &str| {
        p.input(&UiTickInput {
            composing: text.to_string(),
            ..UiTickInput::default()
        });
    };
    assert!(!texts(&p).contains(&"nihao".to_string()));
    compose("nihao");
    let t = texts(&p);
    assert!(t.contains(&"nihao".to_string()), "{t:?}");
    // It is drawn where the caret is: after the two typed characters.
    let [x, y, _, _] = p.rect("email_field");
    let at = draws(&p)
        .into_iter()
        .find_map(|d| match d {
            UiDraw::Text { pos, text, .. } if text == "nihao" => Some(pos),
            _ => None,
        })
        .unwrap();
    assert!(
        (at[0] - (x + 3.0 + 12.0)).abs() < 1.5 && at[1] >= y,
        "{at:?}"
    );
    // It is not text: the field still says "ab" and its length is 2.
    assert_eq!(p.text("email_field"), "ab");
    // Committing it (the characters arrive as typing) clears it.
    compose("");
    assert!(!texts(&p).contains(&"nihao".to_string()));
    // With no field focused there is nowhere to show it.
    p.click_at([1.0, 1.0]);
    compose("zzz");
    assert!(!texts(&p).contains(&"zzz".to_string()));
}

// -- the layout page ------------------------------------------------------------

fn laid() -> Playground {
    let p = Playground::new().unwrap();
    p.show_page("LAYOUT");
    p
}

#[test]
fn margins_limits_percentages_ratio_and_offsets_place_the_boxes() {
    let p = laid();
    let [ax, _, aw, _] = p.rect("margin_a");
    let [bx, _, bw, _] = p.rect("margin_b");
    assert_eq!(aw, 24.0);
    assert_eq!(
        bx - (ax + aw),
        2.0 + 10.0,
        "A's right margin plus B's left margin"
    );
    assert_eq!(bw, 24.0);
    assert_eq!(
        p.rect("limit_fill")[2],
        40.0,
        "a filling box stops at its max width"
    );
    let [_, _, w, _] = p.rect("percent_row");
    let content = w - 4.0 - 2.0 * 1.0; // padding 2 each side, borders do not count
    let (a, b) = (p.rect("pct_a")[2], p.rect("pct_b")[2]);
    assert!(
        (a - content * 0.25).abs() < 4.0 && (b - content * 0.5).abs() < 4.0,
        "{a} {b} {content}"
    );
    let [_, _, rw, rh] = p.rect("ratio");
    assert_eq!((rw, rh), (60.0, 30.0), "2:1");
    let [sx, _, _, _] = p.rect("self_start");
    let [cx, _, cw, _] = p.rect("self_center");
    let [ex, _, ew, _] = p.rect("self_end");
    let [gx, _, gw, _] = p.rect("selfs");
    assert!(sx < cx && cx < ex);
    assert!(
        (ex + ew - (gx + gw)).abs() <= 3.0,
        "end hugs the right edge"
    );
    assert!(
        (cx + cw / 2.0 - (gx + gw / 2.0)).abs() <= 2.0,
        "center is centered"
    );
    let [mx, _, _, _] = p.rect("model");
    assert!(p.rect("shifted")[0] > mx + 12.0, "offset moved it right");
}

#[test]
fn a_wrapping_row_breaks_into_lines_and_the_grid_spans_cells() {
    let p = laid();
    let ys: std::collections::BTreeSet<i32> = (0..10)
        .map(|k| p.rect(&format!("chip_ids[{k}]"))[1] as i32)
        .collect();
    assert!(ys.len() >= 2, "ten chips do not fit one line: {ys:?}");
    let [rx, _, rw, _] = p.rect("chips");
    for k in 0..10 {
        let [x, _, w, _] = p.rect(&format!("chip_ids[{k}]"));
        assert!(x + w <= rx + rw + 1.0, "chip {k} stays inside the row");
    }
    // The grid: the wide cell spans columns 1 and 2, D is in column 3.
    let [wx, wy, ww, _] = p.rect("cell_wide");
    let [ax, ay, _, _] = p.rect("cell_a");
    let [bx, _, bw, _] = p.rect("cell_b");
    let [dx, dy, _, _] = p.rect("cell_d");
    assert_eq!(wx, ax);
    assert!(
        (ww - (bx + bw - ax)).abs() <= 1.0,
        "it covers columns 1 and 2"
    );
    assert!(wy > ay && dy == wy && dx > bx + bw - 1.0);
}

#[test]
fn a_table_sorts_by_a_header_and_selects_a_row() {
    let p = laid();
    let names = |p: &Playground| p.eval::<Vec<String>>("[r[0] for r in ui.table_values(table)]");
    assert_eq!(names(&p), ["ADA", "BO", "CY", "DI"]);
    let header = |i: usize| format!("table.children[{i}]");
    p.click(&header(1)); // LEVEL ascending: 7 9 12 31
    assert_eq!(names(&p), ["BO", "DI", "ADA", "CY"]);
    p.click(&header(1)); // descending
    assert_eq!(names(&p), ["CY", "ADA", "DI", "BO"]);
    p.click(&header(2)); // GOLD ascending (numbers sort as numbers: 85 < 340 < 560 < 1200)
    assert_eq!(names(&p), ["CY", "ADA", "DI", "BO"]);
    p.click(&header(0)); // NAME ascending
    assert_eq!(names(&p), ["ADA", "BO", "CY", "DI"]);
    // Selecting a row.
    p.click("ui.table_cells(table)[2][1]");
    assert_eq!(p.eval::<i32>("ui.table_selected(table)"), 2);
    assert_eq!(p.text("table_said"), "SELECTED: CY");
    p.click("ui.table_cells(table)[2][1]");
    assert_eq!(p.eval::<i32>("ui.table_selected(table)"), -1);
    assert_eq!(p.text("table_said"), "SELECTED: NONE");
}

#[test]
fn lists_have_markers_and_wrapping_items() {
    let p = laid();
    let t = texts(&p);
    assert!(t.contains(&"1.".to_string()) && t.contains(&"3.".to_string()));
    assert_eq!(t.iter().filter(|s| *s == "-").count(), 3, "three bullets");
    let [_, _, cw, _] = p.rect("bullets[0].parent");
    let [x, _, w, _] = p.rect("bullets[1]");
    let [cx, _, _, _] = p.rect("bullets[0].parent");
    assert!(
        x + w <= cx + cw + 1.0,
        "the long item wraps inside the card"
    );
}

// ---- P5: containers and forms ------------------------------------------------------

fn paneled() -> Playground {
    let p = Playground::new().unwrap();
    p.show_page("PANELS");
    p
}

fn shown(p: &Playground, var: &str) -> bool {
    p.eval::<u32>(&format!("{var}.get(Display).none")) == 0
}

#[test]
fn tabs_show_one_panel_and_a_close_button_hides_a_tab() {
    let p = paneled();
    assert_eq!(p.eval::<i32>("ui.selected_tab(tab_root)"), 0);
    assert!(shown(&p, "tab_panels[0]") && !shown(&p, "tab_panels[1]"));
    p.click("tab_buttons_[1]");
    assert_eq!(p.eval::<i32>("ui.selected_tab(tab_root)"), 1);
    assert!(!shown(&p, "tab_panels[0]") && shown(&p, "tab_panels[1]"));
    assert_eq!(p.eval::<u32>("tab_buttons_[1].get(Toggle).on"), 1);
    assert_eq!(p.eval::<u32>("tab_buttons_[0].get(Toggle).on"), 0);
    // Clicking the selected tab keeps it selected.
    p.click("tab_buttons_[1]");
    assert_eq!(p.eval::<u32>("tab_buttons_[1].get(Toggle).on"), 1);
    // Close the second tab: it goes, and the first is selected again.
    p.click("tab_buttons_[1].parent.children[1]");
    assert_eq!(p.text("tab_said"), "CLOSED: TAB 2");
    assert!(!shown(&p, "tab_buttons_[1].parent"));
    assert_eq!(p.eval::<i32>("ui.selected_tab(tab_root)"), 0);
    assert!(shown(&p, "tab_panels[0]") && !shown(&p, "tab_panels[1]"));
}

#[test]
fn dragging_the_divider_resizes_the_panes_within_their_minimums() {
    let p = paneled();
    let ratio = |p: &Playground| p.eval::<f32>("ui.split_ratio(split_root)");
    assert!((ratio(&p) - 0.4).abs() < 1e-4);
    let h = p.eval::<Vec<f32>>(
        "[split_root.children[1].get(Rect).x + 1, split_root.children[1].get(Rect).y + 10]",
    );
    let drag = |p: &Playground, from: [f32; 2], to: [f32; 2]| {
        for (pos, down) in [(from, true), (to, true), (to, false)] {
            p.input(&UiTickInput {
                pointer: Some(pos),
                down,
                ..UiTickInput::default()
            });
        }
    };
    let [x0, _, w0, _] = p.rect("split_a");
    drag(&p, [h[0], h[1]], [h[0] + 30.0, h[1]]);
    let [_, _, w1, _] = p.rect("split_a");
    assert!(ratio(&p) > 0.4 && w1 > w0 + 20.0, "{w0} -> {w1}");
    // Far left: the first pane stops at its minimum of 30.
    let [hx, hy, _, _] = p.rect("split_root.children[1]");
    drag(&p, [hx + 1.0, hy + 10.0], [x0 - 200.0, hy + 10.0]);
    let [_, _, w2, _] = p.rect("split_a");
    assert!((w2 - 30.0).abs() < 1.5, "{w2}");
    // The same through the script, to either end.
    p.run("ui.split_set(split_root, 1.0)");
    p.idle(2);
    assert!(p.rect("split_b")[2] <= 1.0);
}

#[test]
fn details_open_and_close_with_their_header() {
    let p = paneled();
    assert!(!p.eval::<bool>("ui.details_open(details_root)") && !shown(&p, "details_body"));
    p.click("details_root.children[0]");
    assert!(p.eval::<bool>("ui.details_open(details_root)") && shown(&p, "details_body"));
    assert!(texts(&p).contains(&"- MORE INFORMATION".to_string()));
    // The height grows over a few ticks to the body's full height.
    let h = |p: &Playground| p.rect("details_body")[3];
    let reveal = |p: &Playground| p.eval::<f32>("details_body.get(Reveal).amount");
    let mid = h(&p);
    p.idle(12);
    let full = h(&p);
    assert!(full > 5.0 && mid < full, "{mid} -> {full}");
    assert_eq!(reveal(&p), 1.0);
    // Closing shrinks it and then hides it.
    p.click("details_root.children[0]");
    p.idle(3);
    assert!(h(&p) > 0.0 && h(&p) < full, "mid-way: {}", h(&p));
    p.idle(12);
    assert!(!shown(&p, "details_body"));
    assert_eq!(reveal(&p), 0.0);
}

#[test]
fn a_tree_expands_branches_and_selects_one_row() {
    let p = paneled();
    assert!(!texts(&p).contains(&"PLAYER".to_string()), "collapsed");
    // The arrow of WORLD, then of ENTITIES.
    p.click("ui.tree_arrows(tree_root)[0]");
    p.idle(2);
    assert!(texts(&p).contains(&"ENTITIES".to_string()));
    assert!(!texts(&p).contains(&"PLAYER".to_string()));
    p.click("ui.tree_arrows(tree_root)[1]");
    p.idle(2);
    assert!(texts(&p).contains(&"PLAYER".to_string()));
    p.click(
        "[r for r in ui.tree_rows(tree_root) if world.string(r.get(Label).text) == 'PLAYER'][0]",
    );
    assert_eq!(p.text("tree_said"), "SELECTED: PLAYER");
    // Selecting another row deselects the first.
    p.click(
        "[r for r in ui.tree_rows(tree_root) if world.string(r.get(Label).text) == 'LIGHTS'][0]",
    );
    assert_eq!(p.text("tree_said"), "SELECTED: LIGHTS");
    let on = p.eval::<u32>("sum(r.get(Toggle).on for r in ui.tree_rows(tree_root))");
    assert_eq!(on, 1);
    p.run("ui.tree_expand(tree_root, 'WORLD', False)");
    assert!(!texts(&p).contains(&"LIGHTS".to_string()));
}

#[test]
fn radio_buttons_are_exclusive_and_a_label_flips_its_control() {
    let p = paneled();
    assert_eq!(p.eval::<i32>("ui.radio_value(radio_root)"), 1);
    p.click("radios[2]");
    assert_eq!(p.eval::<i32>("ui.radio_value(radio_root)"), 2);
    assert_eq!(p.text("radio_said"), "CHOSEN: LARGE");
    assert_eq!(
        p.eval::<Vec<u32>>("[r.get(Toggle).on for r in radios]"),
        [0, 0, 1]
    );
    // Clicking the chosen one does not unchoose it.
    p.click("radios[2]");
    assert_eq!(p.eval::<i32>("ui.radio_value(radio_root)"), 2);
    // The text next to a radio button chooses it.
    p.click("radios[0].parent.children[1]");
    assert_eq!(p.eval::<i32>("ui.radio_value(radio_root)"), 0);
    // <label for>: the text flips a switch and focuses a field; neither the
    // label nor the click leaves focus on the label itself.
    assert_eq!(p.eval::<u32>("label_switch.get(Toggle).on"), 0);
    p.click("label_text");
    assert_eq!(p.eval::<u32>("label_switch.get(Toggle).on"), 1);
    p.click("label_name_text");
    assert_eq!(focus(&p), p.eval::<u64>("label_name.sid"));
    p.type_text("ok");
    assert_eq!(p.text("label_name"), "ok");
}

#[test]
fn a_form_validates_shows_errors_and_submits_or_resets() {
    let p = paneled();
    let error = |f: &str| {
        p.eval::<String>(&format!(
            "world.string(world.lookup({f}.get(ui.FormField).error).get(Label).text)"
        ))
    };
    p.click("form_ok");
    assert_eq!(p.text("form_said"), "1 PROBLEMS");
    assert_eq!(error("form_name"), "REQUIRED");
    assert!(shown(&p, "world.lookup(form_name.get(ui.FormField).error)"));
    // A one-letter name is too short; a bad code shows the custom message.
    p.click("form_name");
    p.type_text("A");
    p.click("form_code");
    p.type_text("xyz");
    p.click("form_ok");
    assert_eq!(p.text("form_said"), "2 PROBLEMS");
    assert_eq!(error("form_name"), "AT LEAST 2 CHARACTERS");
    assert_eq!(error("form_code"), "USE ABC-12");
    // Fix both and submit.
    p.click("form_name");
    p.type_text("da");
    p.click("form_code");
    p.press(NAV_SELECT_ALL);
    p.type_text("ABC-12");
    p.click("form_ok");
    assert_eq!(p.text("form_said"), "SUBMITTED ADA");
    assert_eq!(p.eval::<u32>("form_root.get(ui.Form).errors"), 0);
    assert_eq!(
        p.eval::<String>("ui.form_values(form_root)['code']"),
        "ABC-12"
    );
    // A number field clamps to its range when it settles, so 200 is 120.
    p.idle(2);
    p.click("form_age");
    p.press(NAV_SELECT_ALL);
    p.type_text("200");
    p.click("form_ok");
    assert_eq!(p.text("form_age"), "120");
    // Reset puts the first values back and hides the errors.
    p.click("form_reset");
    assert_eq!(p.text("form_name"), "");
    assert_eq!(p.text("form_age"), "30");
}

#[test]
fn a_template_puts_the_callers_children_in_its_slots() {
    let p = paneled();
    assert_eq!(p.eval::<usize>("len(slot_body.children)"), 1);
    assert_eq!(p.eval::<usize>("len(slot_footer.children)"), 1);
    assert_eq!(p.text("slot_body.children[0]"), "THE CALLER FILLS THIS.");
    let [_, by, _, _] = p.rect("slot_body");
    let [_, fy, _, _] = p.rect("slot_footer");
    assert!(by < fy, "the body slot sits above the footer slot");
    assert!(p.eval::<bool>("ui.slot_of(slot_card, 'nope') is None"));
}

#[test]
fn the_panels_page_survives_a_save_game() {
    let a = paneled();
    a.click("tab_buttons_[2]");
    a.click("radios[0]");
    a.click("details_root.children[0]");
    a.run("ui.split_set(split_root, 0.7)");
    a.idle(2);
    let text = a.ecs.save_game(Default::default());
    let c = Playground::new().unwrap();
    c.idle(9);
    c.ecs.load_game(&text, Policy::Strict).unwrap();
    assert_eq!(hash(&c), hash(&a));
    assert_eq!(c.eval::<i32>("ui.selected_tab(tab_root)"), 2);
    assert_eq!(c.eval::<i32>("ui.radio_value(radio_root)"), 0);
    assert!(c.eval::<bool>("ui.details_open(details_root)"));
    assert!((c.eval::<f32>("ui.split_ratio(split_root)") - 0.7).abs() < 1e-4);
    for _ in 0..10 {
        a.idle(1);
        c.idle(1);
    }
    assert_eq!(hash(&c), hash(&a));
    assert_eq!(
        c.render(SIZE).to_png_bytes().unwrap(),
        a.render(SIZE).to_png_bytes().unwrap()
    );
}

// ---- P6: drag and drop, resize, shortcuts, buttons -------------------------------

fn dragged() -> Playground {
    let p = Playground::new().unwrap();
    p.show_page("DRAG");
    p
}

#[test]
fn dragging_a_row_onto_another_reorders_the_list() {
    let p = dragged();
    let items = |p: &Playground| p.eval::<Vec<String>>("ui.sortable_items(sort_list)");
    assert_eq!(items(&p), ["ALPHA", "BRAVO", "CHARLIE", "DELTA"]);
    let row = |k: usize| format!("sort_list.children[{k}]");
    p.drag(p.center(&row(0)), p.center(&row(2)));
    assert_eq!(items(&p), ["BRAVO", "CHARLIE", "ALPHA", "DELTA"]);
    assert_eq!(p.text("sort_said"), "MOVED: 1 TO 3");
    // The other way, and a drop on itself changes nothing.
    p.drag(p.center(&row(3)), p.center(&row(0)));
    assert_eq!(items(&p), ["DELTA", "BRAVO", "CHARLIE", "ALPHA"]);
    p.drag(p.center(&row(1)), p.center(&row(1)));
    assert_eq!(items(&p), ["DELTA", "BRAVO", "CHARLIE", "ALPHA"]);
}

#[test]
fn a_drop_zone_takes_only_the_kinds_it_accepts_and_lights_while_a_drag_is_over_it() {
    let p = dragged();
    assert!(!shown(&p, "bin_hint"), "starts hidden");
    // Gems fit the bin.
    let to = p.center("bin_box");
    p.drag(p.center("gem_b"), to);
    assert_eq!(p.text("bin_text"), "BIN: GEM 2");
    assert_eq!(p.text("drag_log"), "LAST: DROPPED");
    p.drag(p.center("gem_a"), to);
    assert_eq!(p.text("bin_text"), "BIN: GEM 1 GEM 2");
    // The coin is another kind: nothing lands, the drag just ends.
    p.drag(p.center("coin"), to);
    assert_eq!(p.text("bin_text"), "BIN: GEM 1 GEM 2");
    assert_eq!(p.text("drag_log"), "LAST: LET GO");
    // The hint shows while a gem is held over the bin, and not for a coin.
    let c = p.center("gem_a");
    for (pos, down) in [(c, true), ([c[0], c[1] + 5.0], true), (to, true)] {
        p.input(&UiTickInput {
            pointer: Some(pos),
            down,
            ..UiTickInput::default()
        });
    }
    p.idle(0);
    assert_eq!(p.eval::<u32>("bin_box.get(DropZone).over"), 1);
    assert!(shown(&p, "bin_hint"));
    p.input(&UiTickInput {
        pointer: Some(to),
        ..UiTickInput::default()
    });
    p.idle(1);
    assert!(!shown(&p, "bin_hint"));
}

#[test]
fn the_grip_resizes_a_panel_down_to_its_minimum() {
    let p = dragged();
    let [_, _, w0, h0] = p.rect("size_root");
    assert_eq!((w0, h0), (120.0, 60.0));
    let grip = "size_root.children[1].children[0]";
    p.drag(
        p.center(grip),
        [p.center(grip)[0] + 30.0, p.center(grip)[1] + 20.0],
    );
    let [_, _, w1, h1] = p.rect("size_root");
    assert_eq!((w1, h1), (150.0, 80.0));
    // Far up and left: it stops at 60x30.
    let c = p.center(grip);
    p.drag(c, [c[0] - 400.0, c[1] - 400.0]);
    let [_, _, w2, h2] = p.rect("size_root");
    assert_eq!((w2, h2), (60.0, 30.0));
}

#[test]
fn key_chords_click_their_widget_and_a_middle_click_is_its_own_event() {
    let p = dragged();
    let (key, mods) = p.eval::<(u32, u32)>("ui.parse_chord('Ctrl+S')");
    assert_eq!((key, mods), (b'S' as u32, 1));
    p.chord(key, mods);
    assert_eq!(p.text("key_said"), "SAVED 1 TIMES");
    p.chord(key, mods);
    assert_eq!(p.text("key_said"), "SAVED 2 TIMES");
    // The wrong modifier does nothing.
    p.chord(key, 0);
    assert_eq!(p.text("key_said"), "SAVED 2 TIMES");
    // F2 flips the switch, Alt+1 presses a button.
    let f2 = p.eval::<(u32, u32)>("ui.parse_chord('F2')");
    p.chord(f2.0, f2.1);
    assert_eq!(p.eval::<u32>("f2_switch.get(Toggle).on"), 1);
    assert_eq!(p.text("key_said"), "SWITCH ON");
    let alt1 = p.eval::<(u32, u32)>("ui.parse_chord('alt + 1')");
    p.chord(alt1.0, alt1.1);
    assert_eq!(p.text("key_said"), "ONE PRESSED");
    assert!(p.eval::<bool>("not ui.parse_chord('shift+ctrl+F12')[1] == 0"));
    // A middle click reports MiddleClicked and is not a primary click.
    let focus_before = focus(&p);
    p.middle_click_at(p.center("mouse_button"));
    assert_eq!(p.text("mouse_said"), "MOUSE: MIDDLE CLICK");
    assert_eq!(focus(&p), focus_before, "no focus from the middle button");
}

#[test]
fn the_drag_page_survives_a_save_game() {
    let a = dragged();
    let row = |k: usize| format!("sort_list.children[{k}]");
    a.drag(a.center(&row(0)), a.center(&row(3)));
    a.drag(a.center("gem_a"), a.center("bin_box"));
    a.chord(b'S' as u32, 1);
    a.idle(2);
    let text = a.ecs.save_game(Default::default());
    let c = Playground::new().unwrap();
    c.idle(9);
    c.ecs.load_game(&text, Policy::Strict).unwrap();
    assert_eq!(hash(&c), hash(&a));
    assert_eq!(
        c.eval::<Vec<String>>("ui.sortable_items(sort_list)"),
        ["BRAVO", "CHARLIE", "DELTA", "ALPHA"]
    );
    assert_eq!(c.text("bin_text"), "BIN: GEM 1");
    // A drag that was in flight when the world was saved continues.
    let b = dragged();
    let start = b.center("gem_b");
    b.input(&UiTickInput {
        pointer: Some(start),
        down: true,
        ..UiTickInput::default()
    });
    let to = b.center("bin_box");
    b.input(&UiTickInput {
        pointer: Some(to),
        down: true,
        ..UiTickInput::default()
    });
    let text = b.ecs.save_game(Default::default());
    let d = Playground::new().unwrap();
    d.idle(9);
    d.ecs.load_game(&text, Policy::Strict).unwrap();
    for q in [&b, &d] {
        q.input(&UiTickInput {
            pointer: Some(to),
            ..UiTickInput::default()
        });
    }
    assert_eq!(d.text("bin_text"), "BIN: GEM 2");
    assert_eq!(hash(&b), hash(&d));
}

#[test]
fn a_recorded_mouse_session_replays_to_the_same_state_every_tick() {
    let a = Playground::new().unwrap();
    a.start_recording(1);
    a.show_page("DRAG");
    let row = |k: usize| format!("sort_list.children[{k}]");
    a.drag(a.center(&row(0)), a.center(&row(2)));
    a.drag(a.center("gem_b"), a.center("bin_box"));
    a.chord(b'S' as u32, 1);
    a.middle_click_at(a.center("mouse_button"));
    a.wheel_at([40.0, 40.0], 1.5);
    a.type_text("q");
    a.idle(3);
    let end = hash(&a);
    let rec = a.take_recording().unwrap();
    assert!(rec.has_pointer());
    let text = rec.to_text();
    assert!(text.starts_with("sage-input v2 "), "{}", &text[..40]);
    assert!(
        text.contains(" w=1.5") && text.contains(" c=53:1") && text.contains(" t=71"),
        "{text}"
    );
    assert_eq!(
        sage_core::input::InputRecording::from_text(&text).unwrap(),
        rec
    );
    // A fresh playground replays it and ends in the same world.
    let b = Playground::new().unwrap();
    let hashes = b.replay(&rec);
    assert_eq!(hashes.len() as u64, rec.len);
    assert_eq!(*hashes.last().unwrap(), end);
    assert_eq!(
        b.eval::<Vec<String>>("ui.sortable_items(sort_list)"),
        ["BRAVO", "CHARLIE", "ALPHA", "DELTA"]
    );
    assert_eq!(b.text("bin_text"), "BIN: GEM 2");
    assert_eq!(b.text("key_said"), "SAVED 1 TIMES");
    assert_eq!(b.text("mouse_said"), "MOUSE: MIDDLE CLICK");
    // A second replay gives the same hash at every tick.
    assert_eq!(Playground::new().unwrap().replay(&rec), hashes);
}

// ---- P7: canvas and viewport -------------------------------------------------------

fn canvased() -> Playground {
    let p = Playground::new().unwrap();
    p.show_page("CANVAS");
    p
}

fn triangles(p: &Playground) -> Vec<[[f32; 2]; 3]> {
    draws(p)
        .into_iter()
        .filter_map(|d| match d {
            UiDraw::Triangle { pts, .. } => Some(pts),
            _ => None,
        })
        .collect()
}

#[test]
fn a_canvas_draws_its_shapes_and_cuts_them_off_at_its_box() {
    let p = canvased();
    // 5 grid lines, 6 chart lines, 7 dots, a triangle, a rectangle (2) and the
    // line that runs off the box.
    assert_eq!(p.eval::<u32>("chart_count"), 5 + 6 + 7 + 1 + 2 + 1);
    let [x, y, w, h] = p.rect("chart");
    assert_eq!((w, h), (200.0, 80.0));
    let t = triangles(&p);
    assert!(t.len() > 50, "lines, dots and triangles: {}", t.len());
    // Nothing the chart drew is outside its rectangle (the movers' canvas is
    // below it, so look only at what starts inside the chart).
    let [_, my, _, _] = p.rect("mover");
    for tri in t.iter().filter(|t| t[0][1] < my - 1.0) {
        for q in tri {
            assert!(
                q[0] >= x - 0.01
                    && q[0] <= x + w + 0.01
                    && q[1] >= y - 0.01
                    && q[1] <= y + h + 0.01,
                "{q:?} is outside the chart"
            );
        }
    }
    // The picture: the green triangle's middle is green, a gap is not.
    let img = p.render(SIZE);
    let green = pixel(&img, [x + 187.0, y + 70.0]);
    assert!(green[1] > green[0] && green[1] > 150, "{green:?}");
    let blank = pixel(&img, [x + 100.0, y + 78.0]);
    assert!(luma(blank) < 40.0, "{blank:?}");
}

#[test]
fn a_script_redraws_a_canvas_every_tick_without_making_new_entities() {
    let p = canvased();
    p.idle(3);
    let count = |p: &Playground| p.eval::<usize>("len(mover.children)");
    let n = count(&p);
    let [_, my, _, mh] = p.rect("mover");
    let ball = |p: &Playground| {
        draws(p)
            .into_iter()
            .filter_map(|d| match d {
                UiDraw::Triangle { pts, colors, .. }
                    if colors[0] == [0xF2, 0x8C, 0x28, 0xFF]
                        && pts[0][1] >= my
                        && pts[0][1] <= my + mh =>
                {
                    Some(pts[0][0])
                }
                _ => None,
            })
            .fold(f32::MAX, f32::min)
    };
    let x0 = ball(&p);
    p.idle(40);
    let x1 = ball(&p);
    assert!(x1 > x0 + 30.0, "the marker moved: {x0} -> {x1}");
    assert_eq!(count(&p), n, "the same shapes are reused");
    assert_eq!(p.eval::<u32>("ui.shape_count(mover)"), 3);
}

#[test]
fn a_viewport_shows_the_hosts_simulation_and_forwards_the_pointer_to_it() {
    let p = canvased();
    let [vx, vy, vw, vh] = p.rect("view");
    // The ball is drawn where the simulation has it.
    let ball = |p: &Playground| {
        draws(p).into_iter().find_map(|d| match d {
            UiDraw::Rect {
                pos,
                size,
                color: [255, 200, 60, 255],
            } => Some((pos, size)),
            _ => None,
        })
    };
    assert!(ball(&p).is_some(), "the ball is on screen");
    assert!(
        texts(&p).iter().any(|t| t.starts_with("TICK ")),
        "the HUD line"
    );
    let (scale, origin) = sage_ui::draw::viewport_fit(
        sage_ui::layout::Rect {
            x: vx,
            y: vy,
            w: vw,
            h: vh,
        },
        Bouncer::WORLD,
    );
    let to_screen = |w: [f32; 2]| {
        [
            origin[0] + w[0] * scale,
            origin[1] + (Bouncer::WORLD[1] - w[1]) * scale,
        ]
    };
    assert_eq!(p.text("view_said"), "POINTER: OUTSIDE");
    // Hover: the pointer is reported in world units, y up.
    let at = to_screen([40.0, 30.0]);
    p.hover(at);
    let (x, y, over, down) = p.eval::<(f32, f32, bool, u32)>("ui.viewport_pointer(view)");
    assert!(over && down == 0);
    assert!(
        (x - 40.0).abs() < 0.01 && (y - 30.0).abs() < 0.01,
        "{x} {y}"
    );
    assert_eq!(p.text("view_said"), "POINTER: OVER");
    // Press: the simulation's ball follows the pointer.
    p.input(&UiTickInput {
        pointer: Some(at),
        down: true,
        ..UiTickInput::default()
    });
    p.input(&UiTickInput {
        pointer: Some(at),
        down: true,
        ..UiTickInput::default()
    });
    let pos = p.bouncer.borrow().pos;
    assert!(
        (pos[0] - 40.0).abs() < 0.01 && (pos[1] - 30.0).abs() < 0.01,
        "{pos:?}"
    );
    assert_eq!(p.text("view_said"), "POINTER: OVER  PRESSED");
    // Drag out of the view: the pointer is still forwarded, past the edge.
    let out = [vx + vw + 30.0, vy + 5.0];
    p.input(&UiTickInput {
        pointer: Some(out),
        down: true,
        ..UiTickInput::default()
    });
    let (x, _, over, _) = p.eval::<(f32, f32, bool, u32)>("ui.viewport_pointer(view)");
    assert!(
        over && x > Bouncer::WORLD[0],
        "dragged past the right edge: {x}"
    );
    // Let go: it flies on by itself, and the picture follows.
    p.input(&UiTickInput {
        pointer: Some(out),
        ..UiTickInput::default()
    });
    let before = p.bouncer.borrow().pos;
    p.idle(5);
    assert_ne!(p.bouncer.borrow().pos, before);
    assert_eq!(p.text("view_said"), "POINTER: OUTSIDE");
    // The view keeps the world's proportions and sits inside its box.
    let (pos, size) = ball(&p).unwrap();
    assert!(
        pos[0] >= vx && pos[0] + size[0] <= vx + vw && pos[1] >= vy && pos[1] + size[1] <= vy + vh
    );
}

#[test]
fn the_canvas_page_survives_a_save_game() {
    let a = canvased();
    a.idle(25);
    let text = a.ecs.save_game(Default::default());
    let c = Playground::new().unwrap();
    c.idle(9);
    c.ecs.load_game(&text, Policy::Strict).unwrap();
    assert_eq!(hash(&c), hash(&a));
    for _ in 0..30 {
        a.idle(1);
        c.idle(1);
        assert_eq!(
            hash(&c),
            hash(&a),
            "the canvas keeps animating the same way"
        );
    }
    assert_eq!(
        c.eval::<u32>("ui.shape_count(chart)"),
        a.eval::<u32>("ui.shape_count(chart)")
    );
}

// ---- P8: semantics -----------------------------------------------------------------

fn access_page(name: &str) -> (Playground, String) {
    let p = Playground::new().unwrap();
    p.show_page(name);
    let outline = p.access_outline();
    (p, outline)
}

#[test]
fn the_python_role_list_is_the_rust_one() {
    let p = Playground::new().unwrap();
    let names = p.eval::<Vec<String>>("ui.ROLE_NAMES");
    assert_eq!(names.len(), 42);
    for (k, name) in names.iter().enumerate() {
        assert_eq!(sage_ui::access::Role::from_u32(k as u32).name(), name);
    }
}

#[test]
fn landmarks_headings_groups_labels_and_alt_text_reach_the_tree() {
    let (p, out) = access_page("ACCESS");
    let want = "\
  banner
    heading \"CASTLE GAME\" level=1
    navigation \"SITE\"
      button \"PLAY\"
      button \"HELP\"
  separator
  main
    region \"SETTINGS\"
      group \"AUDIO\"
        switch \"SOUND\" [off]
        text \"VOLUME\"
        slider \"VOLUME\" value=\"0\"
  contentinfo
    text \"MADE WITH SAGE\"
";
    assert!(out.contains(want), "{out}");
    assert!(out.contains("img \"A GOLD STAR\""), "{out}");
    assert_eq!(
        out.matches("img").count(),
        1,
        "the decoration (alt empty) is skipped"
    );
    assert!(
        out.contains("text \"SCORE\" desc=\"YOUR POINTS SO FAR\""),
        "{out}"
    );
    // The page switcher is a tab list, the page a tab panel named by its tab.
    assert!(
        out.starts_with("heading \"UI PLAYGROUND\" level=1\ntablist \"PAGES\"\n"),
        "{out}"
    );
    assert!(
        out.contains("tab \"ACCESS\" [on]") && out.contains("tabpanel \"ACCESS\"\n"),
        "{out}"
    );
    // Other pages are display: none and absent.
    assert!(!out.contains("PRIMARY") && !out.contains("form"), "{out}");
    // The tree is plain reading: asking twice gives the same answer.
    assert_eq!(p.access_outline(), out);
}

#[test]
fn controls_are_named_by_their_labels_and_report_their_state() {
    let (p, out) = access_page("PANELS");
    for line in [
        "tab \"ONE\" [on]",
        "button \"CLOSE TWO\"",
        "treeitem \"WORLD\" level=1 [off]",
        "radiogroup",
        "radio \"MEDIUM\" [on]",
        "switch \"ENABLE SOUND\" [off]",
        "textbox \"FOCUS THE NAME\" value=\"\"",
        "form",
        "textbox \"NAME *\" value=\"\"",
        "textbox \"AGE\" value=\"30\"",
        "button \"SUBMIT\"",
        "button \"MORE INFORMATION\" [off]",
        "separator",
    ] {
        assert!(
            out.lines().any(|l| l.trim_start() == line),
            "{line} in\n{out}"
        );
    }
    // A label that names a control is not read a second time, and the hidden
    // collapsed branch is not in the tree.
    assert!(
        !out.lines().any(|l| l.trim_start().starts_with("label ")),
        "{out}"
    );
    assert!(
        !out.contains("PLAYER") && !out.contains("HIDDEN UNTIL OPENED"),
        "{out}"
    );
    // The state follows the UI: open the branch, flip the switch, focus a field.
    p.click("ui.tree_arrows(tree_root)[0]");
    p.idle(2);
    p.click("label_text");
    p.click("label_name");
    p.type_text("ada");
    let out = p.access_outline();
    assert!(out.contains("treeitem \"ENTITIES\" level=2 [off]"), "{out}");
    assert!(out.contains("button \"EXPAND WORLD\" [on]"), "{out}");
    assert!(out.contains("switch \"ENABLE SOUND\" [on]"), "{out}");
    assert!(
        out.contains("textbox \"FOCUS THE NAME\" value=\"ada\" [focused]"),
        "{out}"
    );
    // The one focused node is the focused entity.
    let tree = sage_ui::access_tree(&p.world, &p.ecs);
    let id = sage_ui::access::find(&tree, p.eval::<u64>("label_name.sid")).unwrap();
    assert!(id.focused);
}

#[test]
fn a_password_never_exposes_its_text_and_a_disabled_button_says_so() {
    let (p, _) = access_page("TEXT");
    p.click("password_field");
    p.type_text("hunter2");
    let out = p.access_outline();
    assert!(!out.contains("hunter2"), "{out}");
    assert!(
        out.contains("textbox \"PASSWORD\" value=\"\" [focused]"),
        "{out}"
    );
    let q = Playground::new().unwrap();
    assert!(
        q.access_outline()
            .contains("button \"DISABLED\" [disabled]")
    );
}

#[test]
fn popups_are_in_the_tree_only_while_they_are_open() {
    let p = Playground::new().unwrap();
    p.show_page("POPUPS");
    let closed = p.access_outline();
    assert!(
        !closed.contains("listbox") && !closed.contains("dialog") && !closed.contains("menu \""),
        "{closed}"
    );
    assert!(closed.contains("combobox"), "{closed}");
    p.click("color_select");
    p.idle(2);
    let open = p.access_outline();
    assert!(open.contains("listbox"), "{open}");
    assert!(
        open.lines()
            .filter(|l| l.trim_start().starts_with("option "))
            .count()
            >= 2,
        "{open}"
    );
    p.press(NAV_CANCEL);
    p.idle(2);
    assert!(!p.access_outline().contains("listbox"));
    // A dialog is a dialog, with the buttons in it.
    let b = Playground::new().unwrap();
    b.click("dialog_button");
    let out = b.access_outline();
    assert!(out.contains("dialog"), "{out}");
}

// ---- P9: pickers, media ------------------------------------------------------------

fn picking() -> Playground {
    let p = Playground::new().unwrap();
    p.show_page("PICKERS");
    p
}

#[test]
fn calendar_math_is_right_for_known_dates() {
    let p = Playground::new().unwrap();
    // 1970-01-01 Thursday, 2000-01-01 Saturday, 2024-02-29 Thursday,
    // 2026-11-11 Wednesday.
    let days = p.eval::<Vec<u32>>("[ui.weekday(1970,1,1), ui.weekday(2000,1,1), ui.weekday(2024,2,29), ui.weekday(2026,11,11)]");
    assert_eq!(days, [4, 6, 4, 3]);
    let months = p.eval::<Vec<u32>>("[ui.days_in_month(2024,2), ui.days_in_month(2023,2), ui.days_in_month(1900,2), ui.days_in_month(2000,2), ui.days_in_month(2026,12)]");
    assert_eq!(months, [29, 28, 28, 29, 31]);
    assert!(p.eval::<bool>("ui.is_leap(2000) and not ui.is_leap(1900) and ui.is_leap(2024)"));
}

#[test]
fn a_calendar_opens_pages_through_months_and_picks_a_day() {
    let p = picking();
    let date = |p: &Playground| p.eval::<(i32, u32, u32)>("ui.date_value(date_pick)");
    assert_eq!(date(&p), (2026, 11, 11));
    assert_eq!(p.text("date_said"), "DATE: 2026-11-11");
    let popup = "world.lookup(date_pick.get(ui.DateState).popup)";
    assert!(!p.eval::<bool>(&format!("ui.is_open({popup})")));
    p.click("date_pick");
    p.idle(2);
    assert!(p.eval::<bool>(&format!("ui.is_open({popup})")));
    let t = texts(&p);
    assert!(t.contains(&"NOVEMBER 2026".to_string()), "{t:?}");
    // November 2026 starts on a Sunday: the 1st is the first day cell, and the
    // 11th is the one that is selected.
    let cell =
        |d: u32| format!("world.lookup(date_pick.get(ui.DateState).grid).children[7 + {d} - 1]");
    assert_eq!(p.text(&cell(1)), "1");
    assert_eq!(p.eval::<u32>(&format!("{}.get(Toggle).on", cell(11))), 1);
    assert_eq!(p.eval::<u32>(&format!("{}.get(Toggle).on", cell(12))), 0);
    // Next month: December starts on a Tuesday (two blank cells first).
    p.click("world.lookup(date_pick.get(ui.DateState).popup).children[0].children[0].children[2]");
    p.idle(2);
    assert!(texts(&p).contains(&"DECEMBER 2026".to_string()));
    let first = p.eval::<usize>("ui.weekday(2026, 12, 1)");
    assert_eq!(first, 2);
    assert_eq!(
        p.text("world.lookup(date_pick.get(ui.DateState).grid).children[7]"),
        ""
    );
    // Pick the 25th: the field, the event and the popup all agree.
    let k = first + 25 - 1;
    p.click(&format!(
        "world.lookup(date_pick.get(ui.DateState).grid).children[7 + {k}]"
    ));
    assert_eq!(date(&p), (2026, 12, 25));
    assert_eq!(p.text("date_said"), "DATE: 2026-12-25");
    assert!(
        !p.eval::<bool>(&format!("ui.is_open({popup})")),
        "choosing a day closes it"
    );
    // Stepping back past January wraps the year.
    p.click("date_pick");
    p.idle(2);
    let prev =
        "world.lookup(date_pick.get(ui.DateState).popup).children[0].children[0].children[0]";
    for _ in 0..12 {
        p.click(prev);
        p.idle(1);
    }
    assert!(texts(&p).contains(&"DECEMBER 2025".to_string()));
    p.run("ui.date_set(date_pick, 2024, 2, 29)");
    assert_eq!(p.eval::<usize>("ui.days_in_month(2024, 2)"), 29);
    assert_eq!(date(&p), (2024, 2, 29));
}

#[test]
fn a_time_field_clamps_each_part_and_reports() {
    let p = picking();
    assert_eq!(p.eval::<(u32, u32)>("ui.time_value(time_pick)"), (9, 30));
    let hours = "world.lookup(time_pick.get(ui.TimeState).hours)";
    p.click(hours);
    p.press(NAV_SELECT_ALL);
    p.type_text("31");
    p.press(NAV_ACCEPT);
    assert_eq!(
        p.eval::<(u32, u32)>("ui.time_value(time_pick)"),
        (23, 30),
        "31 clamps to 23"
    );
    assert_eq!(p.text("time_said"), "TIME: 23:30");
    let minutes = "world.lookup(time_pick.get(ui.TimeState).minutes)";
    p.click(minutes);
    p.press(NAV_SELECT_ALL);
    p.type_text("5");
    p.press(NAV_ACCEPT);
    assert_eq!(p.text("time_said"), "TIME: 23:05");
}

#[test]
fn a_color_is_picked_by_dragging_the_square_the_sliders_or_typing_hex() {
    let p = picking();
    let color = |p: &Playground| p.eval::<u32>("ui.color_value(color_pick)");
    assert_eq!(color(&p), 0xF28C28FF);
    let popup = "world.lookup(color_pick.get(ui.ColorState).popup)";
    p.click("color_pick");
    p.idle(2);
    assert!(p.eval::<bool>(&format!("ui.is_open({popup})")));
    let grip = "world.lookup(color_pick.get(ui.ColorState).area).children[2]";
    let [gx, gy, gw, gh] = p.rect(grip);
    // The top-left corner of the square is white (no saturation, full value).
    p.drag([gx + 1.0, gy + 1.0], [gx + 1.0, gy + 1.0]);
    let c = color(&p);
    assert!(
        (c >> 24) > 0xF0 && ((c >> 16) & 255) > 0xF0 && ((c >> 8) & 255) > 0xF0,
        "{c:08X}"
    );
    // The bottom is black whatever the hue.
    p.drag(
        [gx + gw / 2.0, gy + gh - 0.5],
        [gx + gw / 2.0, gy + gh - 0.5],
    );
    assert!(
        (color(&p) >> 24) < 0x10 && ((color(&p) >> 8) & 255) < 0x10,
        "{:08X}",
        color(&p)
    );
    // A pure hue at the top-right; the hue slider turns it to green.
    p.drag([gx + gw - 0.5, gy + 0.5], [gx + gw - 0.5, gy + 0.5]);
    p.run("ui.color_set(color_pick, 0xFF0000FF)");
    let hue = "world.lookup(color_pick.get(ui.ColorState).hue)";
    let [hx, hy, hw, hh] = p.rect(hue);
    p.click_at([hx + hw / 3.0, hy + hh / 2.0]);
    let c = color(&p);
    let (r, g, b) = ((c >> 24) & 255, (c >> 16) & 255, (c >> 8) & 255);
    assert!(
        g > 200 && r < 40 && b < 40 || g > 200 && r < 40,
        "a third of the way round is green: {c:08X}"
    );
    // Alpha at the left is transparent.
    let [ax, ay, _, ah] = p.rect("world.lookup(color_pick.get(ui.ColorState).alpha)");
    p.click_at([ax + 0.5, ay + ah / 2.0]);
    assert!(color(&p) & 255 < 4, "{:08X}", color(&p));
    // Typing hex in the field sets every part.
    let hex = "world.lookup(color_pick.get(ui.ColorState).hex)";
    p.click(hex);
    p.press(NAV_SELECT_ALL);
    p.type_text("3366CC");
    assert_eq!(color(&p), 0x3366CCFF);
    assert_eq!(p.text("picked_color"), "COLOR: 3366CCFF");
    assert!(
        p.eval::<f32>("color_pick.get(ui.ColorState).h") > 0.55
            && p.eval::<f32>("color_pick.get(ui.ColorState).h") < 0.65
    );
    // A half typed value changes nothing.
    p.press(NAV_END);
    p.press(NAV_BACKSPACE);
    p.press(NAV_BACKSPACE);
    p.press(NAV_BACKSPACE);
    assert_eq!(color(&p), 0x3366CCFF);
    let (parsed, bad) = (
        p.eval::<u32>("ui.parse_hex('#10203040')"),
        p.eval::<bool>("ui.parse_hex('12345') is None and ui.parse_hex('zzzzzz') is None"),
    );
    assert_eq!(parsed, 0x10203040);
    assert!(bad);
    let rt = p.eval::<bool>("all(ui.pack(*ui.hsv_to_rgb(*ui.rgb_to_hsv(*ui.unpack(c)[:3]))) >> 8 == c >> 8 for c in (0xF28C28FF, 0x000000FF, 0xFFFFFFFF, 0x3366CCFF, 0x12AB34FF))");
    assert!(rt, "hsv and rgb round-trip");
}

#[test]
fn a_file_is_picked_from_a_tree_of_paths() {
    let p = picking();
    assert_eq!(
        p.eval::<Vec<String>>("[str(x) for x in ui.file_tree(['b/c.txt', 'a.txt', 'b/a/d.txt'])]")
            .len(),
        2
    );
    assert_eq!(p.text("file_said"), "FILE: NONE");
    p.click("file_pick");
    p.idle(2);
    let tree = "world.lookup(file_pick.get(ui.FileState).tree)";
    let arrow = |k: usize| format!("ui.tree_arrows({tree})[{k}]");
    // Folders first: art, sounds; then README.txt.
    assert_eq!(
        p.eval::<Vec<String>>(&format!(
            "[world.string(r.get(Label).text) for r in ui.tree_rows({tree}) if True]"
        ))[..2],
        ["art", "checker.png"]
    );
    p.click(&arrow(1)); // sounds
    p.idle(2);
    p.click(&arrow(2)); // sounds/music
    p.idle(2);
    let row = |name: &str| {
        format!(
            "[r for r in ui.tree_rows({tree}) if world.string(r.get(Label).text) == '{name}'][0]"
        )
    };
    p.click(&row("theme.wav"));
    assert_eq!(
        p.eval::<String>("ui.file_value(file_pick)"),
        "sounds/music/theme.wav"
    );
    assert_eq!(p.text("file_said"), "FILE: SOUNDS/MUSIC/THEME.WAV");
    assert!(!p.eval::<bool>("ui.is_open(world.lookup(file_pick.get(ui.FileState).popup))"));
    // Picking a folder only selects it: nothing changes.
    p.click("file_pick");
    p.idle(2);
    p.click(&row("art"));
    assert_eq!(
        p.eval::<String>("ui.file_value(file_pick)"),
        "sounds/music/theme.wav"
    );
}

#[test]
fn the_pickers_page_survives_a_save_game() {
    let a = picking();
    a.run("ui.color_set(color_pick, 0x336699CC)");
    a.run("ui.date_set(date_pick, 2025, 3, 14)");
    a.idle(3);
    let text = a.ecs.save_game(Default::default());
    let c = Playground::new().unwrap();
    c.idle(9);
    c.ecs.load_game(&text, Policy::Strict).unwrap();
    assert_eq!(hash(&c), hash(&a));
    assert_eq!(c.eval::<u32>("ui.color_value(color_pick)"), 0x336699CC);
    assert_eq!(
        c.eval::<(i32, u32, u32)>("ui.date_value(date_pick)"),
        (2025, 3, 14)
    );
}

fn mediaed() -> Playground {
    let p = Playground::new().unwrap();
    p.show_page("MEDIA");
    p
}

fn media(p: &Playground, var: &str) -> (bool, f32, f32, f32) {
    p.eval(&format!("ui.media_state({var})"))
}

#[test]
fn media_controls_play_pause_stop_at_the_end_and_replay_from_the_start() {
    let p = mediaed();
    let button = "world.lookup(audio.get(ui.MediaParts).play)";
    assert_eq!(media(&p, "audio"), (false, 0.0, 120.0, 1.0));
    assert_eq!(p.text(button), "PLAY");
    p.click(button);
    p.idle(1);
    assert_eq!(p.text(button), "PAUSE");
    p.idle(58);
    let (playing, at, _, _) = media(&p, "audio");
    assert!(playing && (58.0..=64.0).contains(&at), "a second in: {at}");
    p.idle(4);
    assert_eq!(
        p.text("world.lookup(audio.get(ui.MediaParts).time)"),
        "0:01 / 0:02"
    );
    // Pause keeps the place.
    p.click(button);
    p.idle(1);
    let held = media(&p, "audio").1;
    p.idle(10);
    assert_eq!(media(&p, "audio").1, held);
    assert_eq!(p.text(button), "PLAY");
    // Play to the end: it stops there, and the next press starts over.
    p.click(button);
    p.idle(100);
    let (playing, at, len, _) = media(&p, "audio");
    assert!(!playing && at == len, "{playing} {at}");
    assert_eq!(
        p.text("world.lookup(audio.get(ui.MediaParts).time)"),
        "0:02 / 0:02"
    );
    p.click(button);
    let (playing, at, _, _) = media(&p, "audio");
    assert!(playing && at < 5.0, "{at}");
}

#[test]
fn the_scrubber_and_volume_slider_move_the_player() {
    let p = mediaed();
    let scrub = "world.lookup(audio.get(ui.MediaParts).scrub)";
    let [x, y, w, h] = p.rect(scrub);
    p.click_at([x + w * 0.5, y + h / 2.0]);
    let at = media(&p, "audio").1;
    assert!((at - 60.0).abs() <= 2.0, "half way through 120 ticks: {at}");
    // The scrubber follows the position as it plays.
    p.click("world.lookup(audio.get(ui.MediaParts).play)");
    p.idle(20);
    let v = p.eval::<f32>(&format!("{scrub}.get(Slider).value"));
    assert!((v - media(&p, "audio").1).abs() < 0.01);
    let vol = "world.lookup(audio.get(ui.MediaParts).volume)";
    let [vx, vy, vw, vh] = p.rect(vol);
    p.click_at([vx + vw * 0.25, vy + vh / 2.0]);
    let volume = media(&p, "audio").3;
    assert!((volume - 0.25).abs() < 0.06, "{volume}");
}

#[test]
fn the_host_plays_the_sound_the_player_says_and_follows_pause_and_scrub() {
    let p = mediaed();
    let loud = |p: &Playground| p.audio(480).iter().any(|s| s.abs() > 0.01);
    assert_eq!(p.voice_count(), 0, "nothing plays before the button");
    p.click("world.lookup(audio.get(ui.MediaParts).play)");
    p.idle(3);
    assert_eq!(p.voice_count(), 1);
    assert!(loud(&p), "the tone is audible");
    // Pause silences it without dropping the voice.
    p.click("world.lookup(audio.get(ui.MediaParts).play)");
    p.idle(2);
    assert!(!loud(&p) && p.voice_count() == 1);
    // Volume zero is silent too.
    p.click("world.lookup(audio.get(ui.MediaParts).play)");
    p.idle(2);
    assert!(loud(&p));
    p.run("ui.media_seek(audio, 118)");
    p.idle(4);
    p.audio(48_000 / 15);
    p.idle(2);
    assert_eq!(p.voice_count(), 0, "the sound reached its end");
    assert!(!media(&p, "audio").0 || media(&p, "audio").1 >= 118.0);
}

#[test]
fn a_video_shows_the_frame_for_its_time_and_loops() {
    let p = mediaed();
    let frame = |p: &Playground| p.eval::<u32>("video_picture.get(Image).frame");
    assert_eq!(frame(&p), 0);
    let (playing, _, len, _) = media(&p, "video");
    assert!(
        !playing && len == 60.0,
        "four frames at four a second is one second"
    );
    p.run("ui.media_play(video)");
    p.idle(16);
    assert_eq!(frame(&p), 1, "a frame lasts 15 ticks");
    p.idle(15);
    assert_eq!(frame(&p), 2);
    p.idle(15);
    assert_eq!(frame(&p), 3);
    p.idle(15);
    assert_eq!(frame(&p), 0, "it loops");
    assert!(media(&p, "video").0, "and keeps playing");
}

#[test]
fn an_svg_document_draws_its_shapes_and_a_bad_one_shows_magenta() {
    let p = mediaed();
    let [x, y, w, h] = p.rect("icon");
    assert_eq!((w, h), (120.0, 60.0));
    let img = p.render(SIZE);
    let heart = pixel(&img, [x + 30.0, y + 24.0]);
    assert!(
        heart[0] > 200 && (100..180).contains(&heart[1]) && heart[2] < 80,
        "orange heart {heart:?}"
    );
    let star = pixel(&img, [x + 85.0, y + 24.0]);
    assert!(star[2] > 150 && star[0] < 120, "teal star {star:?}");
    let dot = pixel(&img, [x + 56.0, y + 51.0]);
    assert!(dot[1] > 150, "the green dot is an arc path {dot:?}");
    let gap = pixel(&img, [x + 112.0, y + 30.0]);
    assert!(luma(gap) < 50.0, "the rounded panel is dark {gap:?}");
    // The corner of the rounded rect is outside the panel: the card shows.
    let corner = pixel(&img, [x + 0.7, y + 0.7]);
    assert_eq!(
        corner,
        pixel(&img, [x - 3.0, y - 3.0]),
        "rx rounds the corner"
    );
    // A size is the document's own when none is given, and alt reaches the tree.
    p.run("world.tiny = ui.svg(svgcard, '<svg viewBox=\"0 0 30 12\"><rect width=\"30\" height=\"12\"/></svg>', alt='A BAR')");
    p.idle(2);
    assert_eq!(p.rect("world.tiny")[2..], [30.0, 12.0]);
    assert!(p.access_outline().contains("img \"A BAR\""));
    // A document that does not parse is loud, not blank.
    p.run("world.bad = ui.svg(svgcard, '<svg><path d=\"M0 0 L5\"/></svg>', width=12, height=12)");
    p.idle(2);
    let [bx, by, _, _] = p.rect("world.bad");
    let img = p.render(SIZE);
    assert_eq!(pixel(&img, [bx + 6.0, by + 6.0]), [255, 0, 255]);
}

#[test]
fn the_media_page_survives_a_save_game_and_shows_in_the_access_tree() {
    let a = mediaed();
    a.click("world.lookup(audio.get(ui.MediaParts).play)");
    a.idle(30);
    a.run("ui.media_seek(video, 20)");
    a.idle(3);
    let text = a.ecs.save_game(Default::default());
    let c = Playground::new().unwrap();
    c.idle(9);
    c.ecs.load_game(&text, Policy::Strict).unwrap();
    assert_eq!(hash(&c), hash(&a));
    for _ in 0..20 {
        a.idle(1);
        c.idle(1);
        assert_eq!(hash(&c), hash(&a));
    }
    assert_eq!(media(&c, "audio"), media(&a, "audio"));
    let out = a.access_outline();
    assert!(out.contains("group \"MEDIA PLAYER\""), "{out}");
    assert!(
        out.contains("button \"PLAY OR PAUSE\"") && out.contains("slider \"POSITION\""),
        "{out}"
    );
}

// ---- the last layout and container gaps: order, sticky, two-way scroll, reorder -----------

#[test]
fn a_sticky_header_holds_while_the_list_scrolls_and_shift_wheel_goes_sideways() {
    let p = laid();
    let [sx, sy, sw, sh] = p.rect("sticky_scroll");
    let first = |p: &Playground| p.rect("sticky_first");
    let second = |p: &Playground| p.rect("sticky_second");
    assert_eq!(first(&p)[1], sy);
    // Scroll down 30: the first header would be above the view, but holds.
    p.wheel_at([sx + sw / 2.0, sy + sh / 2.0], 2.0);
    p.idle(1);
    let off = p.eval::<f32>("sticky_scroll.get(Scroll).offset");
    assert!(off > 12.0, "scrolled: {off}");
    assert_eq!(first(&p)[1], sy, "FRUIT stays at the top of the view");
    // The second header arrives and pushes the first away.
    p.wheel_at([sx + sw / 2.0, sy + sh / 2.0], 4.0);
    p.idle(1);
    // The list ends before TOOLS reaches the top, but it is close, and it
    // pushes FRUIT up ahead of it instead of overlapping.
    assert!(
        second(&p)[1] < sy + 12.0,
        "TOOLS is near the top: {:?}",
        second(&p)
    );
    assert!(
        first(&p)[1] + first(&p)[3] <= second(&p)[1] + 0.5,
        "FRUIT was pushed up, not overlapped"
    );
    // Shift and the wheel scroll across: the rows move left, a header too.
    let x0 = p.rect("sticky_scroll.children[1]")[0];
    p.input(&UiTickInput {
        pointer: Some([sx + 10.0, sy + 20.0]),
        wheel: 2.0,
        nav: NAV_SHIFT,
        ..UiTickInput::default()
    });
    p.idle(1);
    assert!(p.eval::<f32>("sticky_scroll.get(ScrollCross).offset") > 10.0);
    assert!(
        p.rect("sticky_scroll.children[1]")[0] < x0 - 10.0,
        "moved sideways"
    );
    // The wheel alone still goes up and down.
    let before = p.eval::<f32>("sticky_scroll.get(Scroll).offset");
    p.wheel_at([sx + 10.0, sy + 20.0], -1.0);
    assert!(p.eval::<f32>("sticky_scroll.get(Scroll).offset") < before);
}

#[test]
fn tabs_and_tree_rows_are_dragged_into_a_new_order_and_the_state_goes_with_them() {
    let p = paneled();
    let bar = "tab_root.children[0]";
    let xs = |p: &Playground| -> Vec<f32> {
        (0..3)
            .map(|k| p.rect(&format!("{bar}.children[{k}]"))[0])
            .collect()
    };
    assert!(xs(&p)[0] < xs(&p)[1] && xs(&p)[1] < xs(&p)[2]);
    // Drag ONE onto THREE: the order is TWO, THREE, ONE.
    p.drag(
        p.center(&format!("{bar}.children[0].children[0]")),
        p.center(&format!("{bar}.children[2].children[0]")),
    );
    p.idle(2);
    let x = xs(&p);
    assert!(x[1] < x[2] && x[2] < x[0], "{x:?}");
    // Selection and panels are tied to the tabs, not to their places.
    assert_eq!(p.eval::<i32>("ui.selected_tab(tab_root)"), 0);
    p.click(&format!("{bar}.children[2].children[0]"));
    assert_eq!(p.eval::<i32>("ui.selected_tab(tab_root)"), 2);
    assert!(shown(&p, "tab_panels[2]"));
    // A plain click on a draggable tab still selects it.
    p.click(&format!("{bar}.children[1].children[0]"));
    assert_eq!(p.eval::<i32>("ui.selected_tab(tab_root)"), 1);
    // The accessibility tree reads in the new order.
    let out = p.access_outline();
    let at = |t: &str| out.find(&format!("tab \"{t}\"")).unwrap();
    assert!(at("TWO") < at("THREE") && at("THREE") < at("ONE"), "{out}");
    // The tree: drag SETTINGS above WORLD at the top level.
    let node = |k: usize| format!("tree_root.children[{k}]");
    let y = |p: &Playground, k: usize| p.rect(&node(k))[1];
    assert!(y(&p, 0) < y(&p, 2));
    p.drag(
        p.center(&format!("{}.children[0].children[1]", node(2))),
        p.center(&format!("{}.children[0].children[1]", node(0))),
    );
    p.idle(2);
    assert!(y(&p, 2) < y(&p, 0), "SETTINGS is now first");
    assert_eq!(
        p.eval::<Vec<String>>(
            "[world.string(r.get(Label).text) for r in ui.tree_rows(tree_root)][:1]"
        ),
        ["WORLD"],
        "creation order is untouched; only the order keys changed"
    );
    // A drag onto a row of another list does nothing.
    let before = y(&p, 2);
    p.run("ui.tree_expand(tree_root, 'ASSETS', True)");
    p.idle(2);
    assert_eq!(y(&p, 2), before);
}

#[test]
fn a_virtual_list_of_a_hundred_thousand_items_is_six_rows() {
    let p = laid();
    let texts_of = |p: &Playground| {
        p.eval::<Vec<String>>(
            "[world.string(ui.virtual_row(big_list, k).get(Label).text) for k in range(6)]",
        )
    };
    assert_eq!(
        texts_of(&p),
        ["ROW 0", "ROW 1", "ROW 2", "ROW 3", "ROW 4", "ROW 5"]
    );
    assert_eq!(
        p.eval::<usize>("len(world.lookup(big_list.get(ui.VirtualState).inner).children)"),
        6
    );
    // The wheel: three notches move the window.
    let [x, y, w, h] = p.rect("big_list");
    p.wheel_at([x + w / 2.0, y + h / 2.0], 3.0);
    p.idle(2);
    let first = p.eval::<i32>("ui.virtual_first(big_list)");
    assert!(first >= 3, "{first}");
    let texts = texts_of(&p);
    assert_eq!(texts[0], format!("ROW {first}"));
    // Each row sits where its item would be in the full list, so the scroll
    // container shows the right thing: row k is at item first + k.
    let rows =
        p.eval::<Vec<f32>>("[ui.virtual_row(big_list, k).get(Absolute).y for k in range(6)]");
    assert_eq!(rows[0], first as f32 * 10.0);
    // A jump to the middle of a million units reuses the same six rows.
    p.run("big_list.set(Scroll(500000.0))");
    p.idle(3);
    assert_eq!(texts_of(&p)[0], "ROW 50000");
    assert_eq!(
        p.eval::<usize>("len(world.lookup(big_list.get(ui.VirtualState).inner).children)"),
        6
    );
    // The end: rows past the last item are blank and hidden from the reader.
    p.run("big_list.set(Scroll(999950.0))");
    p.idle(3);
    let t = texts_of(&p);
    assert_eq!(t[0], "ROW 99995");
    assert_eq!(t[4], "ROW 99999");
    assert_eq!(t[5], "");
    assert!(!p.access_outline().contains("ROW 50000"));
}

#[test]
fn a_virtual_list_can_show_given_items() {
    let p = laid();
    p.run("world.named = ui.virtual_list(bigcard, items=['ADA', 'BO', 'CY', 'DI', 'ED', 'FLO', 'GUS', 'HAL'], rows=3, row_h=10)");
    p.idle(3);
    assert_eq!(
        p.eval::<Vec<String>>(
            "[world.string(ui.virtual_row(world.named, k).get(Label).text) for k in range(4)]"
        ),
        ["ADA", "BO", "CY", "DI"]
    );
    p.run("world.named.set(Scroll(55.0))");
    p.idle(3);
    assert_eq!(
        p.eval::<Vec<String>>(
            "[world.string(ui.virtual_row(world.named, k).get(Label).text) for k in range(4)]"
        ),
        ["FLO", "GUS", "HAL", ""]
    );
}

#[test]
fn the_accesskit_update_of_a_real_page_is_a_consistent_tree() {
    use std::collections::{BTreeMap, BTreeSet};
    let p = Playground::new().unwrap();
    p.show_page("PANELS");
    p.click("label_name");
    let u = sage_ui::access_update(&p.world, &p.ecs);
    let by: BTreeMap<u64, &accesskit::Node> = u.nodes.iter().map(|(id, n)| (id.0, n)).collect();
    assert_eq!(by.len(), u.nodes.len(), "ids are unique");
    // Every child exists, no node has two parents, and everything is reachable.
    let mut parent_of = BTreeSet::new();
    for (id, n) in &u.nodes {
        for c in n.children() {
            assert!(by.contains_key(&c.0), "{id:?} names a missing child {c:?}");
            assert!(parent_of.insert(c.0), "{c:?} has two parents");
        }
    }
    let mut seen = BTreeSet::new();
    let mut stack = vec![u.tree.as_ref().unwrap().root.0];
    while let Some(id) = stack.pop() {
        if seen.insert(id) {
            stack.extend(by[&id].children().iter().map(|c| c.0));
        }
    }
    assert_eq!(
        seen.len(),
        by.len(),
        "every node is reachable from the window"
    );
    // The focused text box is the update's focus and is named by its label.
    let field = p.eval::<u64>("label_name.sid");
    assert_eq!(u.focus.0, field);
    assert_eq!(by[&field].label(), Some("FOCUS THE NAME"));
    assert_eq!(by[&field].role(), accesskit::Role::TextInput);
}

#[test]
fn a_truetype_face_draws_lower_case_and_antialiased_text_when_one_is_given() {
    let Some(bytes) = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
    ]
    .iter()
    .find_map(|p| std::fs::read(p).ok()) else {
        eprintln!("skipped: no monospaced font on this machine");
        return;
    };
    let p = Playground::new().unwrap();
    p.show_page("TEXT");
    p.click("email_field");
    p.type_text("hello");
    let plain = p.render(SIZE);
    p.set_face(bytes).unwrap();
    let ttf = p.render(SIZE);
    assert_ne!(
        plain.rgba, ttf.rgba,
        "a different face is a different picture"
    );
    // The built-in face has only full-strength pixels; a rasterized one has
    // edges in between.
    let levels = |img: &sage_assets::Image| {
        let mut set = std::collections::BTreeSet::new();
        for y in 0..SIZE[1] {
            for x in 0..SIZE[0] {
                set.insert(img.pixel(x, y)[0]);
            }
        }
        set.len()
    };
    assert!(
        levels(&ttf) > levels(&plain) + 20,
        "{} vs {}",
        levels(&ttf),
        levels(&plain)
    );
    // Garbage is refused up front.
    assert!(p.set_face(b"not a font".to_vec()).is_err());
}

#[test]
fn a_native_file_field_asks_the_host_for_its_dialog_and_takes_the_answer() {
    use sage_platform::ui_window::{FileRequest, UiApp};
    let mut p = picking();
    assert_eq!(p.take_file_request(), None, "nothing asked yet");
    p.click("native_pick");
    p.idle(1);
    // The request reaches the host once, as an open dialog.
    assert_eq!(p.take_file_request(), Some(FileRequest { save: false }));
    assert_eq!(p.take_file_request(), None, "it is not asked for twice");
    // While it is open another click does not ask again.
    p.click("native_pick");
    assert_eq!(p.take_file_request(), None);
    // The user chooses a file: the field shows it and says so.
    p.file_chosen(Some("/home/ada/game/level1.scene".to_string()));
    p.idle(2);
    assert_eq!(
        p.eval::<String>("ui.file_value(native_pick)"),
        "/home/ada/game/level1.scene"
    );
    assert_eq!(
        p.text("native_said"),
        "SYSTEM FILE: /home/ada/game/level1.scene"
    );
    // A cancelled dialog changes nothing, and the field can ask again.
    p.click("native_pick");
    assert!(p.take_file_request().is_some());
    p.file_chosen(None);
    p.idle(2);
    assert_eq!(
        p.eval::<String>("ui.file_value(native_pick)"),
        "/home/ada/game/level1.scene"
    );
    p.click("native_pick");
    assert!(p.take_file_request().is_some(), "ready for another");
    // A save field asks for a save dialog.
    p.run("world.saver = ui.file_field(filecard, native=True, save=True)");
    p.idle(2);
    p.click("world.saver");
    assert_eq!(p.take_file_request(), Some(FileRequest { save: true }));
    // The in-engine browser does not touch the host.
    p.click("file_pick");
    p.idle(2);
    assert_eq!(p.take_file_request(), None);
}

#[test]
fn dragging_a_header_grip_gives_a_table_column_a_width() {
    let p = laid();
    let header = |k: usize| p.rect(&format!("table.children[{k}]"));
    let w0 = header(0)[2];
    assert_eq!(
        p.eval::<Vec<String>>("ui.table_columns(table)"),
        ["auto", "auto", "1fr"]
    );
    let grip = "[c for c in table.children if c.has(ui.ColGripState)][0]";
    let [gx, gy, gw, gh] = p.rect(grip);
    assert!(
        gw <= 3.5 && gh > 5.0,
        "the grip is a thin strip: {:?}",
        [gx, gy, gw, gh]
    );
    assert!(
        (gx + gw - (header(0)[0] + w0)).abs() <= 1.5,
        "at the header's right edge"
    );
    p.drag(
        [gx + gw / 2.0, gy + gh / 2.0],
        [gx + gw / 2.0 + 20.0, gy + gh / 2.0],
    );
    p.idle(2);
    let w1 = header(0)[2];
    assert!((w1 - (w0 + 20.0)).abs() <= 1.5, "{w0} -> {w1}");
    assert_eq!(
        p.eval::<Vec<String>>("ui.table_columns(table)")[0]
            .parse::<f32>()
            .map(|v| v.round()),
        Ok((w0 + 20.0).round())
    );
    // Cells follow the column, and a drag too far left stops at the minimum.
    assert!((p.rect("ui.table_cells(table)[0][0]")[2] - w1).abs() <= 1.5);
    let [gx, gy, gw, gh] = p.rect(grip);
    p.drag([gx + gw / 2.0, gy + gh / 2.0], [gx - 500.0, gy + gh / 2.0]);
    p.idle(2);
    assert_eq!(header(0)[2], 16.0);
    // Sorting and selecting still work with the grips in the grid.
    p.click("table.children[1]");
    assert_eq!(
        p.eval::<Vec<String>>("[r[0] for r in ui.table_values(table)]"),
        ["BO", "DI", "ADA", "CY"]
    );
}

#[test]
fn assistive_technology_can_focus_and_press_controls_like_the_pointer_would() {
    use sage_platform::ui_window::UiApp;
    let mut p = Playground::new().unwrap();
    p.show_page("PANELS");
    // Ask through the accesskit request, as the platform adapter would.
    let request = |p: &mut Playground, var: &str, action| {
        let id = p.eval::<u64>(&format!("{var}.sid"));
        p.access_action(accesskit::ActionRequest {
            action,
            target_tree: accesskit::TreeId::ROOT,
            target_node: accesskit::NodeId(id),
            data: None,
        });
        p.idle(2);
    };
    // Click a switch: it flips; a second click flips it back.
    assert_eq!(p.eval::<u32>("label_switch.get(Toggle).on"), 0);
    request(&mut p, "label_switch", accesskit::Action::Click);
    assert_eq!(p.eval::<u32>("label_switch.get(Toggle).on"), 1);
    request(&mut p, "label_switch", accesskit::Action::Click);
    assert_eq!(p.eval::<u32>("label_switch.get(Toggle).on"), 0);
    // Focus a text box and type into it.
    request(&mut p, "label_name", accesskit::Action::Focus);
    assert_eq!(focus(&p), p.eval::<u64>("label_name.sid"));
    p.type_text("hi");
    assert_eq!(p.text("label_name"), "hi");
    // A button's click reaches its observers.
    request(&mut p, "form_ok", accesskit::Action::Click);
    assert_eq!(p.text("form_said"), "1 PROBLEMS");
    // Actions the UI does not do are refused, and a disabled control ignores them.
    assert!(!sage_ui::perform_access_action(
        &p.world,
        1,
        accesskit::Action::ScrollDown
    ));
    p.run("label_switch.set(Widget(2, 0, 0, 0))");
    request(&mut p, "label_switch", accesskit::Action::Click);
    assert_eq!(p.eval::<u32>("label_switch.get(Toggle).on"), 0);
    // The tree the adapter is given is the same one: it names the controls.
    let u = p.access_tree();
    assert!(
        u.nodes
            .iter()
            .any(|(_, n)| n.label() == Some("FOCUS THE NAME"))
    );
}

#[test]
fn a_proportional_face_sizes_labels_by_their_letters_and_leaves_inputs_on_the_grid() {
    let Some(bytes) = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    ]
    .iter()
    .find_map(|p| std::fs::read(p).ok()) else {
        eprintln!("skipped: no proportional font on this machine");
        return;
    };
    let tab_w = |p: &Playground, name: &str| {
        let i = p.eval::<usize>(&format!("page_names.index({name:?})"));
        p.rect(&format!("tab_buttons[{i}]"))[2]
    };
    // On the grid, two four-letter tabs are the same width.
    let p = Playground::new().unwrap();
    p.idle(2);
    assert_eq!(tab_w(&p, "TEXT"), tab_w(&p, "DRAG"));
    let field_w = p.rect("name")[2];
    // With a proportional face they follow their letters, and a longer word
    // of narrow letters can be shorter than a shorter word of wide ones.
    p.set_face_proportional(bytes).unwrap();
    p.idle(2);
    let (t, d) = (tab_w(&p, "TEXT"), tab_w(&p, "DRAG"));
    assert_ne!(t, d, "TEXT {t} and DRAG {d}");
    assert!(
        tab_w(&p, "BASICS") < 6.0 * 6.0 + 12.0,
        "narrower than six grid cells and padding"
    );
    // A text input still lays out on the grid and still edits by columns.
    assert_eq!(p.rect("name")[2], field_w);
    p.click("name");
    p.press(NAV_END);
    p.type_text("W");
    assert_eq!(p.text("name"), "AdaW");
    // Removing the face goes back to the grid.
    sage_core::host::set_text_metrics(None);
}

#[test]
fn bold_and_italic_change_the_glyphs_on_the_built_in_face_and_on_a_truetype_one() {
    use sage_core::host::{STYLE_BOLD, STYLE_ITALIC};
    // The built-in face strikes bold twice (more ink, one unit wider) and has
    // no italic.
    let font = sage_assets::BitmapFont::builtin();
    assert!(!font.styled);
    assert_eq!(
        font.layout_styled("I", [0.0; 2], 1.0, STYLE_ITALIC),
        font.layout("I", [0.0; 2], 1.0)
    );
    // A TrueType face has styled glyphs: bold is heavier than regular, italic
    // leans (its ink is shifted right at the top of the glyph).
    let Some(bytes) = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
        .or_else(|_| std::fs::read("/usr/share/fonts/dejavu/DejaVuSans.ttf"))
        .ok()
    else {
        eprintln!("skipped: no font on this machine");
        return;
    };
    for font in [
        sage_assets::BitmapFont::from_ttf_proportional(&bytes, 3).unwrap(),
        sage_assets::BitmapFont::from_ttf(&bytes, 3).unwrap(),
    ] {
        assert!(font.styled);
        let ink = |c: char, style: u8| -> Vec<(u32, u32)> {
            let q = &font.layout_styled(&c.to_string(), [0.0; 2], 1.0, style)[0];
            let (aw, ah) = (font.atlas.width as f32, font.atlas.height as f32);
            let (x0, y0) = (
                (q.uv_min[0] * aw).round() as u32,
                (q.uv_min[1] * ah).round() as u32,
            );
            (0..font.glyph_px[1])
                .flat_map(|y| (0..font.glyph_px[0]).map(move |x| (x, y)))
                .filter(|&(x, y)| font.atlas.pixel(x0 + x, y0 + y)[3] > 100)
                .collect()
        };
        let (regular, bold, italic, both) = (
            ink('H', 0),
            ink('H', STYLE_BOLD),
            ink('H', STYLE_ITALIC),
            ink('H', STYLE_BOLD | STYLE_ITALIC),
        );
        assert!(
            bold.len() > regular.len() + 10,
            "{} vs {}",
            bold.len(),
            regular.len()
        );
        assert!(both.len() > italic.len() + 10);
        assert_ne!(italic, regular);
        // The slant: the top row's ink sits to the right of the bottom row's.
        let mean = |pts: &[(u32, u32)], top: bool| {
            let y = if top {
                pts.iter().map(|p| p.1).min()
            } else {
                pts.iter().map(|p| p.1).max()
            }
            .unwrap();
            let row: Vec<u32> = pts.iter().filter(|p| p.1 == y).map(|p| p.0).collect();
            row.iter().sum::<u32>() as f32 / row.len() as f32
        };
        let lean = |pts: &[(u32, u32)]| mean(pts, true) - mean(pts, false);
        assert!(
            lean(&italic) > lean(&regular) + 1.0,
            "{} vs {}",
            lean(&italic),
            lean(&regular)
        );
        // Styles do not change what is drawn where: same quad, other texels.
        assert_eq!(
            font.layout_styled("Hi", [0.0; 2], 1.0, STYLE_BOLD).len(),
            font.layout("Hi", [0.0; 2], 1.0).len()
        );
    }
}

#[test]
fn single_asterisks_make_italics_in_markup_and_a_lone_one_stays_an_asterisk() {
    let p = Playground::new().unwrap();
    p.run("world.itl = ui.rich(app, 'A *LEAN* WORD AND **BOLD** AND *BOTH **STRONG** SLANT* END 5*3')");
    p.idle(2);
    let styles: Vec<(String, u8)> = draws(&p)
        .into_iter()
        .filter_map(|d| match d {
            UiDraw::Text { text, style, .. } if style != 0 => Some((text, style)),
            _ => None,
        })
        .collect();
    assert!(styles.contains(&("LEAN".to_string(), 2)), "{styles:?}");
    assert!(styles.contains(&("BOLD".to_string(), 1)), "{styles:?}");
    assert!(
        styles.iter().any(|(t, s)| t.contains("STRONG") && *s == 3),
        "{styles:?}"
    );
    assert!(
        texts(&p).iter().any(|t| t.contains("5*3")),
        "a lone asterisk is drawn"
    );
}
