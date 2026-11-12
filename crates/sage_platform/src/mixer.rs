//! A software mixer: sounds in, interleaved stereo `f32` out.
//!
//! Pure CPU code with no device, so it is tested headless. Audio is
//! presentation (design doc 10.2): the simulation emits cues, this mixes
//! them, and nothing flows back.

use std::rc::Rc;

use sage_assets::Sound;

/// Playing more than this many sounds at once drops the oldest.
pub const MAX_VOICES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceId(pub u64);

struct Voice {
    id: VoiceId,
    sound: Rc<Sound>,
    /// Position in source frames (fractional because of pitch and rate conversion).
    pos: f64,
    volume: f32,
    pitch: f32,
    looping: bool,
    /// A paused voice keeps its place and makes no sound.
    paused: bool,
}

pub struct Mixer {
    out_rate: u32,
    voices: Vec<Voice>,
    next_id: u64,
    /// Master volume, 0 to 1.
    pub master: f32,
}

impl Mixer {
    pub fn new(out_rate: u32) -> Mixer {
        Mixer {
            out_rate,
            voices: vec![],
            next_id: 0,
            master: 1.0,
        }
    }

    pub fn out_rate(&self) -> u32 {
        self.out_rate
    }

    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    /// Starts a sound. `pitch` 1.0 is normal speed.
    pub fn play(&mut self, sound: Rc<Sound>, volume: f32, pitch: f32, looping: bool) -> VoiceId {
        if self.voices.len() >= MAX_VOICES {
            self.voices.remove(0);
        }
        let id = VoiceId(self.next_id);
        self.next_id += 1;
        self.voices.push(Voice {
            id,
            sound,
            pos: 0.0,
            volume,
            pitch: pitch.max(0.01),
            looping,
            paused: false,
        });
        id
    }

    fn voice_mut(&mut self, id: VoiceId) -> Option<&mut Voice> {
        self.voices.iter_mut().find(|v| v.id == id)
    }

    /// Pauses or resumes a voice, keeping its place. False if it is gone.
    pub fn set_paused(&mut self, id: VoiceId, paused: bool) -> bool {
        self.voice_mut(id).map(|v| v.paused = paused).is_some()
    }

    pub fn set_volume(&mut self, id: VoiceId, volume: f32) -> bool {
        self.voice_mut(id).map(|v| v.volume = volume).is_some()
    }

    /// Moves a voice to `seconds` into its sound (clamped to the sound).
    pub fn seek(&mut self, id: VoiceId, seconds: f64) -> bool {
        self.voice_mut(id)
            .map(|v| {
                let last = v.sound.samples.len().saturating_sub(1) as f64;
                v.pos = (seconds * f64::from(v.sound.sample_rate)).clamp(0.0, last);
            })
            .is_some()
    }

    /// Where a voice is in its sound, in seconds; `None` once it is gone.
    pub fn position(&self, id: VoiceId) -> Option<f64> {
        self.voices
            .iter()
            .find(|v| v.id == id)
            .map(|v| v.pos / f64::from(v.sound.sample_rate))
    }

    pub fn is_playing(&self, id: VoiceId) -> bool {
        self.voices.iter().any(|v| v.id == id && !v.paused)
    }

    pub fn stop(&mut self, id: VoiceId) {
        self.voices.retain(|v| v.id != id);
    }

    pub fn stop_all(&mut self) {
        self.voices.clear();
    }

    /// Mixes `frames` stereo frames (2 floats each), advancing every voice
    /// and removing finished ones. Output is clamped to [-1, 1].
    pub fn mix(&mut self, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; frames * 2];
        let out_rate = f64::from(self.out_rate);
        for v in &mut self.voices {
            let len = v.sound.samples.len();
            if len == 0 || v.paused {
                continue;
            }
            let step = f64::from(v.sound.sample_rate) / out_rate * f64::from(v.pitch);
            let gain = v.volume * self.master;
            for frame in out.as_chunks_mut::<2>().0.iter_mut() {
                if v.looping {
                    v.pos %= len as f64;
                } else if v.pos >= len as f64 {
                    break;
                }
                let i = v.pos as usize;
                let frac = (v.pos - i as f64) as f32;
                let a = v.sound.samples[i];
                let b = v.sound.samples[if i + 1 < len {
                    i + 1
                } else if v.looping {
                    0
                } else {
                    i
                }];
                let s = (a + (b - a) * frac) * gain;
                frame[0] += s;
                frame[1] += s;
                v.pos += step;
            }
        }
        self.voices
            .retain(|v| v.paused || v.looping || v.pos < v.sound.samples.len() as f64);
        for s in &mut out {
            *s = s.clamp(-1.0, 1.0);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dc(rate: u32, n: usize, level: f32) -> Rc<Sound> {
        Rc::new(Sound {
            sample_rate: rate,
            samples: vec![level; n],
        })
    }

    #[test]
    fn a_sound_plays_once_and_is_removed() {
        let mut m = Mixer::new(1000);
        m.play(dc(1000, 10, 0.5), 1.0, 1.0, false);
        let out = m.mix(20);
        assert_eq!(&out[..4], &[0.5, 0.5, 0.5, 0.5]);
        assert_eq!(out[18], 0.5); // frame 9, the last sample
        assert_eq!(out[20], 0.0); // frame 10 onwards is silence
        assert_eq!(m.voice_count(), 0);
    }

    #[test]
    fn voices_sum_and_clamp() {
        let mut m = Mixer::new(1000);
        m.play(dc(1000, 4, 0.75), 1.0, 1.0, false);
        m.play(dc(1000, 4, 0.75), 1.0, 1.0, false);
        assert_eq!(m.mix(1), vec![1.0, 1.0]);
    }

    #[test]
    fn volume_master_and_stop() {
        let mut m = Mixer::new(1000);
        m.master = 0.5;
        let id = m.play(dc(1000, 100, 1.0), 0.5, 1.0, true);
        assert_eq!(m.mix(1)[0], 0.25);
        m.stop(id);
        assert_eq!(m.mix(1)[0], 0.0);
    }

    #[test]
    fn rate_conversion_and_pitch_change_how_fast_the_sound_advances() {
        // A ramp 0, 1, 2, 3... played at double speed reads every other sample.
        let ramp = Rc::new(Sound {
            sample_rate: 1000,
            samples: (0..8).map(|i| i as f32 / 8.0).collect(),
        });
        let mut m = Mixer::new(1000);
        m.play(ramp.clone(), 1.0, 2.0, false);
        let out = m.mix(4);
        assert_eq!(out[0], 0.0);
        assert!((out[2] - 2.0 / 8.0).abs() < 1e-6);
        assert!((out[6] - 6.0 / 8.0).abs() < 1e-6);
        // Output at twice the source rate: linear interpolation between samples.
        let mut m = Mixer::new(2000);
        m.play(ramp, 1.0, 1.0, false);
        let out = m.mix(3);
        assert!((out[2] - 0.5 / 8.0).abs() < 1e-6);
    }

    #[test]
    fn loops_wrap_around() {
        let mut m = Mixer::new(1000);
        m.play(
            Rc::new(Sound {
                sample_rate: 1000,
                samples: vec![0.1, 0.2],
            }),
            1.0,
            1.0,
            true,
        );
        let out = m.mix(5);
        let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
        assert_eq!(left, [0.1, 0.2, 0.1, 0.2, 0.1]);
        assert_eq!(m.voice_count(), 1);
    }

    #[test]
    fn the_oldest_voice_is_dropped_at_the_limit() {
        let mut m = Mixer::new(1000);
        let first = m.play(dc(1000, 100, 0.0), 1.0, 1.0, true);
        for _ in 0..MAX_VOICES {
            m.play(dc(1000, 100, 0.0), 1.0, 1.0, true);
        }
        assert_eq!(m.voice_count(), MAX_VOICES);
        m.stop(first); // already gone: stopping again is harmless
        assert_eq!(m.voice_count(), MAX_VOICES);
    }

    #[test]
    fn a_paused_voice_keeps_its_place_and_is_silent_until_resumed() {
        let mut m = Mixer::new(1000);
        let id = m.play(dc(1000, 100, 0.5), 1.0, 1.0, false);
        m.mix(10);
        assert!((m.position(id).unwrap() - 0.010).abs() < 1e-9);
        assert!(m.set_paused(id, true));
        assert!(!m.is_playing(id));
        assert!(m.mix(10).iter().all(|s| *s == 0.0), "silent while paused");
        assert!(
            (m.position(id).unwrap() - 0.010).abs() < 1e-9,
            "it did not move"
        );
        m.set_paused(id, false);
        assert!(m.mix(5).iter().all(|s| *s == 0.5));
        assert!((m.position(id).unwrap() - 0.015).abs() < 1e-9);
    }

    #[test]
    fn seek_moves_a_voice_and_volume_changes_while_it_plays() {
        let mut m = Mixer::new(1000);
        let id = m.play(dc(1000, 100, 0.5), 1.0, 1.0, false);
        assert!(m.seek(id, 0.095));
        let out = m.mix(10);
        assert_eq!(out[0], 0.5);
        assert_eq!(out[10], 0.0, "five frames were left, then silence");
        assert_eq!(m.voice_count(), 0, "it finished");
        assert_eq!(m.position(id), None);
        assert!(!m.seek(id, 0.0) && !m.set_paused(id, true) && !m.set_volume(id, 1.0));
        let id = m.play(dc(1000, 100, 1.0), 1.0, 1.0, true);
        assert!(m.set_volume(id, 0.25));
        assert_eq!(m.mix(1)[0], 0.25);
        m.seek(id, 99.0); // clamped to the last sample
        assert!(m.position(id).unwrap() <= 0.099);
        // A paused voice that reaches nowhere is not collected.
        m.set_paused(id, true);
        m.mix(500);
        assert_eq!(m.voice_count(), 1);
    }
}
