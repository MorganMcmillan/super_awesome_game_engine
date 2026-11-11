use sage_core::host::UiDraw;
use sage_ui::interact::{NAV_ACCEPT, NAV_NEXT, NAV_RIGHT};
use ui_demo::{Demo, SCREEN};

const SCALE: u32 = 4;

fn pixel(img: &sage_assets::Image, ui: [f32; 2]) -> [u8; 4] {
    let (x, y) = ((ui[0] * SCALE as f32) as u32, (ui[1] * SCALE as f32) as u32);
    img.pixel(x, y)
}

#[test]
fn a_session_changes_the_widgets_and_the_picture_shows_it() {
    let demo = Demo::new().unwrap();
    assert_eq!(demo.status(), "Ready");
    demo.play_session();
    // The observers (Python) reacted to the last event of the session.
    assert_eq!(demo.status(), "Music on");

    let img = demo.render([SCREEN[0] as u32 * SCALE, SCREEN[1] as u32 * SCALE]);
    // The slider was set to 7 of 10: accent to 70% of its width, track after.
    let [x, y, w, h] = demo.rect("volume");
    let mid = y + h / 2.0;
    assert_eq!(pixel(&img, [x + w * 0.4, mid]), [0x6A, 0xA9, 0xFF, 0xFF]);
    assert_eq!(pixel(&img, [x + w * 0.9, mid]), [0x2A, 0x2F, 0x3A, 0xFF]);
    // The toggle is on: its center shows the accent.
    let [mx, my, mw, mh] = demo.rect("music");
    assert_eq!(
        pixel(&img, [mx + mw / 2.0, my + mh / 2.0]),
        [0x6A, 0xA9, 0xFF, 0xFF]
    );
    // The typed name is in the draw list (the caret is a bar, not a character).
    let texts: Vec<String> = demo
        .frame()
        .draws
        .iter()
        .filter_map(|d| match d {
            UiDraw::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"Ada".to_string()), "{texts:?}");
    assert!(texts.contains(&"Music on".to_string()), "{texts:?}");
    // Play is hovered: lighter than its resting fill.
    let [px, py, _, _] = demo.rect("play");
    assert_eq!(pixel(&img, [px + 1.0, py + 1.0]), [0x4A, 0x51, 0x60, 0xFF]);
    // The panel's border is drawn at its edge.
    let [bx, by, _, _] = demo.rect("panel");
    assert_eq!(pixel(&img, [bx, by + 20.0]), [0x3A, 0x41, 0x50, 0xFF]);
    // Everything is inside the screen and the PNG encodes.
    assert!(!img.to_png_bytes().unwrap().is_empty());
}

#[test]
fn the_whole_menu_fits_on_the_screen() {
    let demo = Demo::new().unwrap();
    for var in ["panel", "play", "music", "volume", "name", "status"] {
        let [x, y, w, h] = demo.rect(var);
        assert!(
            x >= 0.0 && y >= 0.0 && x + w <= SCREEN[0] && y + h <= SCREEN[1],
            "{var} at {:?} is outside the {SCREEN:?} screen",
            [x, y, w, h]
        );
    }
}

#[test]
fn the_menu_works_from_the_keyboard_and_renders_identically_twice() {
    let render = || {
        let demo = Demo::new().unwrap();
        demo.nav(NAV_NEXT); // Play
        demo.nav(NAV_ACCEPT);
        assert_eq!(demo.status(), "Starting...");
        demo.nav(NAV_NEXT); // Music
        demo.nav(NAV_NEXT); // Volume
        demo.nav(NAV_RIGHT);
        assert_eq!(demo.status(), "Volume 4");
        demo.render([320, 240]).to_png_bytes().unwrap()
    };
    assert_eq!(render(), render(), "the same inputs draw the same bytes");
}
