//! Sounds: decoded to mono `f32` samples in `[-1, 1]`.
//!
//! The decoder reads RIFF/WAVE with PCM (8, 16, 24 and 32 bit) or IEEE float
//! (32 bit) data and mixes any channel count down to mono. Other formats
//! (Ogg, MP3) are out of M1's scope.

#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

impl Sound {
    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate as f32
    }
}

fn u16_at(b: &[u8], i: usize) -> Result<u16, String> {
    b.get(i..i + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| "wav: truncated".to_string())
}

fn u32_at(b: &[u8], i: usize) -> Result<u32, String> {
    b.get(i..i + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| "wav: truncated".to_string())
}

pub fn decode(bytes: &[u8]) -> Result<Sound, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("wav: not a RIFF/WAVE file".into());
    }
    // (format tag, channels, rate, bits)
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32_at(bytes, at + 4)? as usize;
        let body = at + 8;
        let end = (body + size).min(bytes.len());
        match id {
            b"fmt " => {
                let mut tag = u16_at(bytes, body)?;
                let channels = u16_at(bytes, body + 2)?;
                let rate = u32_at(bytes, body + 4)?;
                let bits = u16_at(bytes, body + 14)?;
                if tag == 0xFFFE && size >= 26 {
                    // WAVE_FORMAT_EXTENSIBLE: the real tag starts the sub-format GUID.
                    tag = u16_at(bytes, body + 24)?;
                }
                fmt = Some((tag, channels, rate, bits));
            }
            b"data" => data = Some(&bytes[body..end]),
            _ => {}
        }
        at = body + size + (size & 1); // chunks are padded to even length
    }
    let (tag, channels, rate, bits) = fmt.ok_or("wav: no fmt chunk")?;
    let data = data.ok_or("wav: no data chunk")?;
    if channels == 0 || rate == 0 {
        return Err("wav: zero channels or sample rate".into());
    }
    let bytes_per = usize::from(bits / 8);
    let sample = |s: &[u8]| -> Result<f32, String> {
        Ok(match (tag, bits) {
            (1, 8) => (f32::from(s[0]) - 128.0) / 128.0,
            (1, 16) => f32::from(i16::from_le_bytes([s[0], s[1]])) / 32768.0,
            (1, 24) => (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f32 / 8_388_608.0,
            (1, 32) => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 / 2_147_483_648.0,
            (3, 32) => f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
            _ => return Err(format!("wav: unsupported format tag {tag}, {bits} bits")),
        })
    };
    let frame = bytes_per * usize::from(channels);
    let mut samples = Vec::with_capacity(data.len() / frame.max(1));
    for f in data.chunks_exact(frame) {
        let mut sum = 0.0;
        for c in f.chunks_exact(bytes_per) {
            sum += sample(c)?;
        }
        samples.push(sum / f32::from(channels));
    }
    Ok(Sound {
        sample_rate: rate,
        samples,
    })
}

/// Encodes mono 16-bit PCM.
pub fn encode_pcm16(sound: &Sound) -> Vec<u8> {
    let data_len = (sound.samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sound.sample_rate.to_le_bytes());
    out.extend_from_slice(&(sound.sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in &sound.samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// A square-wave blip with a linear fade-out, for generated placeholder
/// sound effects. Deterministic: no randomness, no wall clock.
pub fn square_blip(sample_rate: u32, freq_hz: f32, secs: f32, volume: f32) -> Sound {
    let n = (sample_rate as f32 * secs) as usize;
    let samples = (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let phase = (t * freq_hz).fract();
            let wave = if phase < 0.5 { 1.0 } else { -1.0 };
            let fade = 1.0 - i as f32 / n as f32;
            wave * fade * volume
        })
        .collect();
    Sound {
        sample_rate,
        samples,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm16_round_trip_is_within_one_step() {
        let s = square_blip(22_050, 440.0, 0.05, 0.5);
        let back = decode(&encode_pcm16(&s)).unwrap();
        assert_eq!(back.sample_rate, s.sample_rate);
        assert_eq!(back.samples.len(), s.samples.len());
        for (a, b) in s.samples.iter().zip(&back.samples) {
            assert!((a - b).abs() < 1.0 / 16384.0);
        }
    }

    #[test]
    fn stereo_is_mixed_down_to_mono() {
        let mut w = encode_pcm16(&Sound {
            sample_rate: 8000,
            samples: vec![0.0; 4],
        });
        // Rewrite as stereo 16-bit: left 0.5, right -0.5 cancel; left 1.0, right 1.0 stay.
        w[22..24].copy_from_slice(&2u16.to_le_bytes());
        let pairs: [i16; 4] = [16384, -16384, 16384, 16384];
        w.truncate(44);
        for p in pairs {
            w.extend_from_slice(&p.to_le_bytes());
        }
        let len = (pairs.len() * 2) as u32;
        w[40..44].copy_from_slice(&len.to_le_bytes());
        let s = decode(&w).unwrap();
        assert_eq!(s.samples.len(), 2);
        assert!(s.samples[0].abs() < 1e-6);
        assert!((s.samples[1] - 0.5).abs() < 1e-4);
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        assert!(decode(b"").is_err());
        assert!(decode(b"RIFF....WAVE").is_err());
        let mut w = encode_pcm16(&square_blip(8000, 100.0, 0.01, 1.0));
        w[20..22].copy_from_slice(&99u16.to_le_bytes()); // unknown format tag
        assert!(decode(&w).is_err());
    }
}
