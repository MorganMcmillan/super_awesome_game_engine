//! The platformer's placeholder sounds, generated in code so nothing is
//! downloaded or licensed: `platformer gen-assets` writes them to
//! `examples/platformer/assets/`, where they are committed so the game loads
//! them like any other asset. A test checks that the committed files match
//! this generator.
//!
//! There is no placeholder art to generate: the kit draws plain colored boxes
//! (design doc 7), and an artist replaces them by giving entities a
//! `Look(image=...)`. The names here are the kit's cue names
//! (`sage_platformer::CUES`), one file each.

use sage_assets::Sound;
use sage_assets::wav;

/// Directory of the committed assets (compile-time path, for the example).
pub const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

const RATE: u32 = 22_050;

#[derive(Clone, Copy)]
enum Wave {
    Square,
    Triangle,
    Noise,
}

/// A tone that glides from `f0` to `f1` with a fade-out. Deterministic: the
/// noise is a fixed linear congruential generator.
fn tone(wave: Wave, f0: f32, f1: f32, secs: f32, volume: f32) -> Sound {
    let n = (RATE as f32 * secs) as usize;
    let (mut phase, mut seed) = (0.0f32, 0x2545_F491u32);
    let samples = (0..n)
        .map(|i| {
            let k = i as f32 / n as f32;
            phase = (phase + (f0 + (f1 - f0) * k) / RATE as f32).fract();
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let w = match wave {
                Wave::Square => {
                    if phase < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
                Wave::Noise => (seed >> 16) as f32 / 32768.0 - 1.0,
            };
            w * (1.0 - k).powf(1.5) * volume
        })
        .collect();
    Sound {
        sample_rate: RATE,
        samples,
    }
}

/// Plays `parts` one after another.
fn chain(parts: &[Sound]) -> Sound {
    Sound {
        sample_rate: RATE,
        samples: parts.iter().flat_map(|s| s.samples.clone()).collect(),
    }
}

/// (cue name, sound) for every cue the kit emits.
pub fn sounds() -> Vec<(&'static str, Sound)> {
    use Wave::*;
    vec![
        ("jump", tone(Square, 260.0, 520.0, 0.14, 0.35)),
        ("land", tone(Noise, 180.0, 60.0, 0.05, 0.25)),
        (
            "wall_jump",
            chain(&[
                tone(Square, 330.0, 330.0, 0.05, 0.3),
                tone(Square, 495.0, 660.0, 0.09, 0.3),
            ]),
        ),
        ("roll", tone(Noise, 400.0, 120.0, 0.16, 0.2)),
        ("dive", tone(Triangle, 700.0, 160.0, 0.22, 0.4)),
        ("spin", tone(Triangle, 400.0, 800.0, 0.18, 0.35)),
        (
            "coin",
            chain(&[
                tone(Square, 988.0, 988.0, 0.05, 0.25),
                tone(Square, 1319.0, 1319.0, 0.14, 0.25),
            ]),
        ),
        ("stomp", tone(Square, 320.0, 110.0, 0.1, 0.4)),
        ("hurt", tone(Square, 220.0, 90.0, 0.3, 0.4)),
        (
            "checkpoint",
            chain(&[
                tone(Triangle, 523.0, 523.0, 0.07, 0.4),
                tone(Triangle, 659.0, 659.0, 0.07, 0.4),
                tone(Triangle, 784.0, 784.0, 0.16, 0.4),
            ]),
        ),
        ("death", tone(Square, 440.0, 55.0, 0.6, 0.4)),
    ]
}

/// (file name, bytes) for every generated asset.
pub fn generate() -> Vec<(String, Vec<u8>)> {
    sounds()
        .into_iter()
        .map(|(name, s)| (format!("{name}.wav"), wav::encode_pcm16(&s)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_is_a_sound_for_every_cue_the_kit_emits() {
        let names: Vec<&str> = sounds().iter().map(|s| s.0).collect();
        for (_, cue) in sage_platformer::CUES {
            assert!(names.contains(&cue), "no sound for the {cue} cue");
        }
        assert_eq!(names.len(), sage_platformer::CUES.len());
    }

    #[test]
    fn committed_assets_match_the_generator() {
        // Run `cargo run -p platformer -- gen-assets` after changing the generator.
        let stale = sage_assets::generated::stale(DIR, &generate());
        assert!(stale.is_empty(), "stale: {stale:?}");
    }

    #[test]
    fn sounds_are_short_audible_and_inside_range() {
        for (name, s) in sounds() {
            assert!((0.04..1.0).contains(&s.duration_secs()), "{name}");
            let peak = s.samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!((0.1..=1.0).contains(&peak), "{name} peak {peak}");
        }
    }
}
