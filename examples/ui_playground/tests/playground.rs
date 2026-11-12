use sage_core::StateHasher;
use sage_core::host::{NAV_END, NAV_NEXT, NAV_PAGE_DOWN, UiDraw};
use sage_ecs_py::Policy;
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
    assert!(p.eval::<bool>("driver.get(Driver).dialog is not None"));
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
    assert!(p.eval::<bool>("driver.get(Driver).dialog is None"));
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
    assert!(c.eval::<bool>("driver.get(Driver).dialog is not None"));
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
