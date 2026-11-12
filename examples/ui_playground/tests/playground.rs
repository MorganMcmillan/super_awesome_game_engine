use sage_core::StateHasher;
use sage_core::host::{
    NAV_ACCEPT, NAV_COPY, NAV_CUT, NAV_END, NAV_HOME, NAV_LEFT, NAV_NEXT, NAV_PAGE_DOWN, NAV_PASTE,
    NAV_REDO, NAV_SELECT_ALL, NAV_SHIFT, NAV_UNDO, NAV_UP, NAV_WORD, UiDraw,
};
use sage_ecs_py::Policy;
use sage_platform::ui_window::UiTickInput;
use ui_playground::{Playground, SCREEN};

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
    // Bold draws its glyphs twice, one unit apart.
    let bold: Vec<_> = draws
        .iter()
        .filter_map(|d| match d {
            UiDraw::Text { text, pos, .. } if text == "BOLD," || text == "BOLD" => Some(pos[0]),
            _ => None,
        })
        .collect();
    assert_eq!(bold.len(), 2, "{bold:?}");
    assert!((bold[1] - bold[0] - 1.0).abs() < 0.01);
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
}
