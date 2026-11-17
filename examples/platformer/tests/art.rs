//! The hand-off to an artist (design doc 7, 10.3): the kit draws plain boxes,
//! and giving an entity a `Look(image=...)` swaps in a sprite with nothing
//! else changed. The image is found by `kit.asset(path)`, the same id the
//! asset store gives that path; a sheet is cut into cells and `frame` picks
//! one.

use platformer::Game;
use sage_assets::{AssetId, Image};
use sage_core::host::UiDraw;
use sage_core::{Simulation, TickInput};
use sage_platform::game_window::{GameApp, render_frame};
use sage_platformer::{Platformer, PlatformerConfig};

/// A game that puts `hero.png` (a sheet of two cells) on the player.
fn hero_game() -> Game {
    let script = format!(
        r#"{}
from sage.platformer import Look
hero = world.lookup(kit.ID_PLAYER)
look = hero.get(Look)
look.image = kit.asset("art/hero.png")
look.cols = 2   # two cells; the kit shows the cell numbered by the player's mode
look.color = 0
hero.set(look)
"#,
        platformer::SCRIPT
    );
    let sim = Platformer::new(PlatformerConfig::new(1, vec![("game".into(), script)])).unwrap();
    // Two cells side by side: red, then blue.
    let mut sheet = Image::solid(2, 1, [255, 0, 0, 255]);
    sheet.set_pixel(1, 0, [0, 0, 255, 255]);
    Game::around(sim).with_image("art/hero.png", sheet)
}

fn player_image(g: &Game) -> (u64, [f32; 2], [f32; 2], [f32; 4]) {
    let frame = g.frame();
    frame
        .draws
        .iter()
        .find_map(|d| match d {
            UiDraw::Image {
                asset,
                pos,
                size,
                uv,
                ..
            } => Some((*asset, *pos, *size, *uv)),
            _ => None,
        })
        .expect("the player is drawn as an image")
}

#[test]
fn the_python_asset_id_is_the_asset_stores_id() {
    let mut g = hero_game();
    g.step(TickInput::EMPTY);
    let (asset, _, size, _) = player_image(&g);
    assert_eq!(asset, AssetId::of_path("art/hero.png").0);
    assert_eq!(size, [10.0, 14.0], "stretched over the collider box");
    // Backslashes and a leading ./ do not change it.
    assert_eq!(
        AssetId::of_path(".\\art\\hero.png"),
        AssetId::of_path("art/hero.png")
    );
}

/// The pixel at the middle of the player's image, in a 1280 by 720 render
/// (the view is 320 by 180, drawn at 4x).
fn middle_pixel(g: &Game) -> [u8; 4] {
    let (_, pos, size, _) = player_image(g);
    let img = render_frame(&g.frame(), &g.images(), [1280, 720]);
    let (cx, cy) = (
        ((pos[0] + size[0] / 2.0) * 4.0) as u32,
        ((pos[1] + size[1] / 2.0) * 4.0) as u32,
    );
    img.pixel(cx, cy)
}

#[test]
fn a_sheet_shows_the_cell_of_the_players_mode() {
    let mut g = hero_game();
    // Standing on the ground is mode 0 (ground): the first cell, red.
    for _ in 0..40 {
        g.step(TickInput::EMPTY);
    }
    assert_eq!(g.sim.mode_name(), "ground");
    assert_eq!(player_image(&g).3, [0.0, 0.0, 0.5, 1.0]);
    assert_eq!(middle_pixel(&g), [255, 0, 0, 255]);
    // In the air is mode 1: the second cell, blue.
    for _ in 0..6 {
        g.step(TickInput {
            buttons: sage_platformer::buttons::JUMP,
        });
    }
    assert_eq!(g.sim.mode_name(), "air");
    assert_eq!(player_image(&g).3, [0.5, 0.0, 1.0, 1.0]);
    assert_eq!(middle_pixel(&g), [0, 0, 255, 255]);
}

#[test]
fn a_sprite_is_not_tinted_by_the_rules_and_turns_with_the_player() {
    let mut g = hero_game();
    for _ in 0..40 {
        g.step(TickInput::EMPTY);
    }
    let tint = |g: &Game| {
        g.frame().draws.iter().find_map(|d| match d {
            UiDraw::Image { tint, .. } => Some(*tint),
            _ => None,
        })
    };
    assert_eq!(tint(&g), Some([255, 255, 255, 255]));
    // Facing left swaps the cell's edges (the sprite mirrors).
    for _ in 0..10 {
        g.step(TickInput {
            buttons: sage_platformer::buttons::LEFT,
        });
    }
    let uv = player_image(&g).3;
    assert!(uv[0] > uv[2], "mirrored: {uv:?}");
}

#[test]
fn plain_boxes_are_what_the_kit_draws_without_art() {
    let mut g = Game::new(1).unwrap();
    g.step(TickInput::EMPTY);
    assert!(
        !g.frame()
            .draws
            .iter()
            .any(|d| matches!(d, UiDraw::Image { .. })),
        "no images until an artist supplies some"
    );
}
