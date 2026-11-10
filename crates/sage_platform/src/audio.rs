//! Audio output: the software mixer feeding one SDL audio stream.
//!
//! The window loop calls [`AudioOut::pump`] once per frame to keep about
//! `TARGET_QUEUE_SECS` of audio queued. Cues are names: the game says
//! "bounce" and the bank maps the name to a decoded sound.

use std::collections::BTreeMap;
use std::rc::Rc;

use sage_assets::Sound;
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};

use crate::mixer::Mixer;

const RATE: u32 = 48_000;
const TARGET_QUEUE_SECS: f32 = 0.06;

pub struct AudioOut {
    stream: AudioStreamOwner,
    mixer: Mixer,
    bank: BTreeMap<String, Rc<Sound>>,
}

impl AudioOut {
    /// Opens the default playback device. Fails (without side effects) on a
    /// machine with no audio device; callers then run silent.
    pub fn open(sdl: &sdl3::Sdl) -> Result<AudioOut, String> {
        let audio = sdl.audio().map_err(|e| e.to_string())?;
        let spec = AudioSpec {
            freq: Some(RATE as i32),
            channels: Some(2),
            format: Some(AudioFormat::f32_sys()),
        };
        let device = audio
            .open_playback_device(&spec)
            .map_err(|e| e.to_string())?;
        let stream = device
            .open_device_stream(Some(&spec))
            .map_err(|e| e.to_string())?;
        stream.resume().map_err(|e| e.to_string())?;
        Ok(AudioOut {
            stream,
            mixer: Mixer::new(RATE),
            bank: BTreeMap::new(),
        })
    }

    pub fn register(&mut self, cue: &str, sound: Rc<Sound>) {
        self.bank.insert(cue.to_string(), sound);
    }

    /// Plays the sound registered under `cue`. Unknown cues are ignored, so a
    /// simulation can name sounds the player has not installed.
    pub fn cue(&mut self, cue: &str) -> bool {
        match self.bank.get(cue) {
            Some(s) => {
                self.mixer.play(s.clone(), 0.6, 1.0, false);
                true
            }
            None => false,
        }
    }

    pub fn mixer(&mut self) -> &mut Mixer {
        &mut self.mixer
    }

    /// Tops up the device queue. Call at least every few frames.
    pub fn pump(&mut self) -> Result<(), String> {
        let queued_frames = self.stream.queued_bytes().map_err(|e| e.to_string())? as usize / 8;
        let want = (RATE as f32 * TARGET_QUEUE_SECS) as usize;
        if queued_frames < want {
            let samples = self.mixer.mix(want - queued_frames);
            self.stream
                .put_data_f32(&samples)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
