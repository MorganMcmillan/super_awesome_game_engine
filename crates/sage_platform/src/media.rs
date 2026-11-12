//! Keeps a mixer voice in step with a UI media player.
//!
//! The UI owns the player's state (`sage_ui::ecs::UiMedia`: playing, position
//! in ticks, volume, looping), because that is what the user clicks and what a
//! save game holds. Audio is presentation, so this binder only reads that
//! state each tick and tells the mixer: start a voice, pause it, change its
//! volume, or seek it when the user moved the scrubber. Nothing flows back.

use std::rc::Rc;

use sage_assets::Sound;
use sage_core::TICK_DT;

use crate::mixer::{Mixer, VoiceId};

/// How far (in ticks) the voice may drift from the UI's position before the
/// binder seeks it: playing normally never gets there, a scrub does.
pub const SEEK_TOLERANCE_TICKS: f64 = 3.0;

/// What the UI says the player is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaState {
    pub playing: bool,
    /// Where the player is, in simulation ticks.
    pub position: f32,
    /// How long the sound is, in ticks. A player stopped at its end has no
    /// further use for its voice.
    pub length: f32,
    pub volume: f32,
    pub looping: bool,
}

/// The length of a sound in ticks, for the player's scrubber.
pub fn length_ticks(sound: &Sound) -> f32 {
    (sound.samples.len() as f64 / f64::from(sound.sample_rate.max(1)) / f64::from(TICK_DT)) as f32
}

/// One player's voice.
#[derive(Debug, Default)]
pub struct MediaVoice {
    voice: Option<VoiceId>,
}

impl MediaVoice {
    pub fn is_sounding(&self, mixer: &Mixer) -> bool {
        self.voice.is_some_and(|v| mixer.is_playing(v))
    }

    /// Brings the mixer in line with `state`. Call it once per tick.
    pub fn sync(&mut self, mixer: &mut Mixer, sound: &Rc<Sound>, state: MediaState) {
        let want = f64::from(state.position) * f64::from(TICK_DT);
        // A player that stopped at its end lets its voice go.
        if !state.playing && state.length > 0.0 && state.position >= state.length {
            self.stop(mixer);
            return;
        }
        // A sound that played to its end is gone from the mixer.
        if let Some(id) = self.voice
            && mixer.position(id).is_none()
        {
            self.voice = None;
        }
        match (state.playing, self.voice) {
            (true, None) => {
                let id = mixer.play(sound.clone(), state.volume, 1.0, state.looping);
                mixer.seek(id, want);
                self.voice = Some(id);
            }
            (playing, Some(id)) => {
                mixer.set_paused(id, !playing);
                mixer.set_volume(id, state.volume);
                let at = mixer.position(id).unwrap_or(want);
                if (at - want).abs() > SEEK_TOLERANCE_TICKS * f64::from(TICK_DT) {
                    mixer.seek(id, want);
                }
            }
            (false, None) => {}
        }
    }

    /// Stops the voice (the player went away).
    pub fn stop(&mut self, mixer: &mut Mixer) {
        if let Some(id) = self.voice.take() {
            mixer.stop(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One second of a constant tone at 600 Hz, so a tick is 10 samples.
    fn tone() -> Rc<Sound> {
        Rc::new(Sound {
            sample_rate: 600,
            samples: vec![0.5; 600],
        })
    }

    fn state(playing: bool, position: f32) -> MediaState {
        MediaState {
            playing,
            position,
            length: 60.0,
            volume: 1.0,
            looping: false,
        }
    }

    fn loud(m: &mut Mixer) -> bool {
        m.mix(10).iter().any(|s| *s != 0.0)
    }

    #[test]
    fn playing_starts_a_voice_pausing_silences_it_and_scrubbing_seeks() {
        let mut m = Mixer::new(600);
        let sound = tone();
        assert!((length_ticks(&sound) - 60.0).abs() < 1e-3);
        let mut v = MediaVoice::default();
        v.sync(&mut m, &sound, state(false, 0.0));
        assert_eq!(m.voice_count(), 0, "paused at the start: nothing plays");
        v.sync(&mut m, &sound, state(true, 0.0));
        assert!(v.is_sounding(&m) && loud(&mut m));
        // The UI's position advances with the voice, so no seek happens.
        v.sync(&mut m, &sound, state(true, 1.0));
        let id = v.voice.unwrap();
        assert!((m.position(id).unwrap() - 1.0 / 60.0).abs() < 1e-6);
        // Pause: silent, and it keeps its place.
        v.sync(&mut m, &sound, state(false, 1.0));
        assert!(!v.is_sounding(&m) && !loud(&mut m));
        // Scrub while paused to 30 ticks, then play: it is half way.
        v.sync(&mut m, &sound, state(false, 30.0));
        assert!((m.position(id).unwrap() - 0.5).abs() < 1e-6);
        v.sync(&mut m, &sound, state(true, 30.0));
        assert!(v.is_sounding(&m) && loud(&mut m));
        // Volume follows the slider.
        let mut quiet = state(true, 31.0);
        quiet.volume = 0.0;
        v.sync(&mut m, &sound, quiet);
        assert!(!loud(&mut m));
    }

    #[test]
    fn a_sound_that_ends_is_started_again_from_the_ui_position() {
        let mut m = Mixer::new(600);
        let sound = tone();
        let mut v = MediaVoice::default();
        v.sync(&mut m, &sound, state(true, 55.0));
        m.mix(60); // past the end: the mixer drops the voice
        assert_eq!(m.voice_count(), 0);
        v.sync(&mut m, &sound, state(false, 60.0));
        assert!(!v.is_sounding(&m));
        // Stopped at the end, a paused voice would linger: it is released.
        v.sync(&mut m, &sound, state(true, 50.0));
        v.sync(&mut m, &sound, state(false, 60.0));
        assert_eq!(m.voice_count(), 0, "released at the end");
        v.sync(&mut m, &sound, state(true, 0.0));
        assert!(v.is_sounding(&m), "play again from the start");
        assert!(m.position(v.voice.unwrap()).unwrap() < 0.01);
        v.stop(&mut m);
        assert_eq!(m.voice_count(), 0);
    }
}
