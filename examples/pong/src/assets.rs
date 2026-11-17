//! Pong's assets, generated in code so no art or audio is downloaded or
//! licensed: `pong gen-assets` writes them to `examples/pong/assets/`, where
//! they are committed so the game loads them like any other asset. A test
//! checks that the committed files match this generator.

use sage_assets::{Image, wav};

/// Directory of the committed assets (compile-time path, for the example).
pub const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

/// The ball in `kind_rgb(Ball)` yellow: an 8x8 disc on a transparent background.
pub fn ball_sprite() -> Image {
    let mut img = Image::solid(8, 8, [255, 200, 60, 0]);
    for y in 0..8u32 {
        for x in 0..8u32 {
            let (dx, dy) = (x as f32 - 3.5, y as f32 - 3.5);
            if dx * dx + dy * dy <= 4.0 * 4.0 {
                img.set_pixel(x, y, [255, 200, 60, 255]);
            }
        }
    }
    img
}

/// (file name, bytes) for every generated asset.
pub fn generate() -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let blip = |hz, secs| wav::encode_pcm16(&wav::square_blip(22_050, hz, secs, 0.5));
    Ok(vec![
        ("ball.png", ball_sprite().to_png_bytes()?),
        ("paddle.wav", blip(440.0, 0.08)),
        ("wall.wav", blip(220.0, 0.06)),
        ("score.wav", blip(150.0, 0.25)),
        ("win.wav", blip(660.0, 0.4)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_assets_match_the_generator() {
        // Run `cargo run -p pong -- gen-assets` after changing the generator.
        let stale = sage_assets::generated::stale(DIR, &generate().unwrap());
        assert!(stale.is_empty(), "stale: {stale:?}");
    }

    #[test]
    fn the_ball_sprite_is_a_disc() {
        let s = ball_sprite();
        assert_eq!(s.pixel(0, 0)[3], 0);
        assert_eq!(s.pixel(4, 4), [255, 200, 60, 255]);
    }
}
