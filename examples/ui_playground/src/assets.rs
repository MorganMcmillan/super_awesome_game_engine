//! The playground's images, drawn in code so no binary files are committed.
//! Each is registered under a path; Python refers to it by that path
//! (`ui.image(parent, "art/logo.png")`).

use sage_assets::{AssetId, Image};

pub const LOGO: &str = "art/logo.png";
pub const FRAME: &str = "art/frame.png";
pub const SHEET: &str = "art/spinner.png";
pub const CHECKER: &str = "art/checker.png";

fn mix(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), m(a[3], b[3])]
}

/// A 32x32 diamond, orange at the top to deep red at the bottom, with a
/// light edge.
pub fn logo() -> Image {
    let mut img = Image::solid(32, 32, [0, 0, 0, 0]);
    for y in 0..32u32 {
        for x in 0..32u32 {
            let d = (x as f32 - 15.5).abs() + (y as f32 - 15.5).abs();
            if d <= 15.0 {
                let c = mix([242, 140, 40, 255], [150, 40, 30, 255], y as f32 / 31.0);
                img.set_pixel(x, y, if d > 13.5 { [255, 220, 170, 255] } else { c });
            }
        }
    }
    img
}

/// A 12x12 frame for 9-slicing with insets of 4: a bright border, dotted
/// corners and a dark center.
pub fn frame() -> Image {
    let mut img = Image::solid(12, 12, [26, 22, 18, 235]);
    for i in 0..12 {
        for (x, y) in [(i, 0), (i, 11), (0, i), (11, i)] {
            img.set_pixel(x, y, [242, 140, 40, 255]);
        }
    }
    for (x, y) in [(2, 2), (9, 2), (2, 9), (9, 9)] {
        img.set_pixel(x, y, [255, 220, 170, 255]);
    }
    img
}

/// A 64x16 sheet of four 16x16 frames: a dot that moves round a square.
pub fn spinner() -> Image {
    let mut img = Image::solid(64, 16, [0, 0, 0, 0]);
    let spots = [(2, 2), (10, 2), (10, 10), (2, 10)];
    for (frame, (sx, sy)) in spots.iter().enumerate() {
        for dy in 0..4 {
            for dx in 0..4 {
                img.set_pixel(frame as u32 * 16 + sx + dx, sy + dy, [242, 140, 40, 255]);
            }
        }
        // A faint ring so every frame shows something.
        for i in 1..15 {
            let c = [60, 50, 40, 255];
            let o = frame as u32 * 16;
            for (x, y) in [(i, 1), (i, 14), (1, i), (14, i)] {
                if img.pixel(o + x, y)[3] == 0 {
                    img.set_pixel(o + x, y, c);
                }
            }
        }
    }
    img
}

/// 8x8, two tones: shows how images tile, tint and clip.
pub fn checker() -> Image {
    let mut img = Image::solid(8, 8, [0, 0, 0, 255]);
    for y in 0..8 {
        for x in 0..8 {
            let on = (x / 2 + y / 2) % 2 == 0;
            img.set_pixel(
                x,
                y,
                if on {
                    [90, 80, 70, 255]
                } else {
                    [40, 34, 28, 255]
                },
            );
        }
    }
    img
}

/// Every image by path and its id.
pub fn all() -> Vec<(&'static str, u64, Image)> {
    [
        (LOGO, logo()),
        (FRAME, frame()),
        (SHEET, spinner()),
        (CHECKER, checker()),
    ]
    .into_iter()
    .map(|(path, image)| (path, AssetId::of_path(path).0, image))
    .collect()
}

pub const TONE: &str = "sounds/tone.wav";

/// Two seconds of a rising tone with a slow beat, so a player has something
/// to scrub through. Presentation only: it is not simulation state.
pub fn tone() -> sage_assets::Sound {
    let rate = 8_000u32;
    let samples = (0..rate * 2)
        .map(|i| {
            let t = i as f32 / rate as f32;
            let freq = 220.0 + 110.0 * t;
            let beat = 0.6 + 0.4 * (t * std::f32::consts::TAU * 2.0).sin().abs();
            (t * freq * std::f32::consts::TAU).sin() * 0.3 * beat
        })
        .collect();
    sage_assets::Sound {
        sample_rate: rate,
        samples,
    }
}
