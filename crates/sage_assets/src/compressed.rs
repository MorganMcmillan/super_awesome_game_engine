//! Compressed sound formats: Ogg Vorbis (through `lewton`) and MP3 (through
//! `puremp3`), decoded to the same mono `f32` [`Sound`] the WAV decoder makes.
//! [`decode_any`] looks at the first bytes and picks the decoder, so a path's
//! extension never decides what a file is.

use std::io::Cursor;

use crate::wav::{self, Sound};

/// Which decoder a file's first bytes call for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Wav,
    Ogg,
    Mp3,
}

/// The format of `bytes`, from its magic number.
pub fn sniff(bytes: &[u8]) -> Option<Format> {
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        Some(Format::Wav)
    } else if bytes.starts_with(b"OggS") {
        Some(Format::Ogg)
    } else if bytes.starts_with(b"ID3")
        || (bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] & 0xE0 == 0xE0)
    {
        Some(Format::Mp3)
    } else {
        None
    }
}

fn mono(interleaved: &[i16], channels: usize) -> Vec<f32> {
    interleaved
        .chunks(channels.max(1))
        .map(|f| f.iter().map(|&s| f32::from(s) / 32768.0).sum::<f32>() / f.len() as f32)
        .collect()
}

/// Decodes Ogg Vorbis, mixing any channel count down to mono.
pub fn decode_ogg(bytes: &[u8]) -> Result<Sound, String> {
    let mut r = lewton::inside_ogg::OggStreamReader::new(Cursor::new(bytes))
        .map_err(|e| format!("ogg: {e}"))?;
    let channels = usize::from(r.ident_hdr.audio_channels);
    let rate = r.ident_hdr.audio_sample_rate;
    let mut samples = vec![];
    while let Some(packet) = r.read_dec_packet_itl().map_err(|e| format!("ogg: {e}"))? {
        samples.extend(mono(&packet, channels));
    }
    if samples.is_empty() {
        return Err("ogg: no audio".into());
    }
    Ok(Sound {
        sample_rate: rate,
        samples,
    })
}

/// Decodes MP3, mixing the channels down to mono.
pub fn decode_mp3(bytes: &[u8]) -> Result<Sound, String> {
    let (header, frames) =
        puremp3::read_mp3(Cursor::new(bytes)).map_err(|e| format!("mp3: {e}"))?;
    // The decoder gets the low-sample-rate variants (MPEG-2 and 2.5) wrong
    // (the pitch comes out double), so those are refused rather than played
    // wrongly.
    if header.version != puremp3::MpegVersion::Mpeg1 {
        return Err("mp3: only MPEG-1 (32, 44.1 and 48 kHz) is supported".into());
    }
    let samples: Vec<f32> = frames.map(|(l, r)| (l + r) / 2.0).collect();
    if samples.is_empty() {
        return Err("mp3: no audio".into());
    }
    Ok(Sound {
        sample_rate: header.sample_rate.hz(),
        samples,
    })
}

/// Decodes WAV, Ogg Vorbis or MP3, whichever `bytes` is.
pub fn decode_any(bytes: &[u8]) -> Result<Sound, String> {
    match sniff(bytes) {
        Some(Format::Wav) => wav::decode(bytes),
        Some(Format::Ogg) => decode_ogg(bytes),
        Some(Format::Mp3) => decode_mp3(bytes),
        None => Err("sound: not a WAV, Ogg or MP3 file".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OGG: &[u8] = include_bytes!("../tests/data/tone440.ogg");
    const OGG_STEREO: &[u8] = include_bytes!("../tests/data/stereo440.ogg");
    const MP3: &[u8] = include_bytes!("../tests/data/tone440.mp3");

    /// The frequency of a tone, from its rising zero crossings.
    fn hz(s: &Sound) -> f32 {
        let crossings = s
            .samples
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        crossings as f32 / s.duration_secs()
    }

    fn rms(s: &Sound) -> f32 {
        (s.samples.iter().map(|x| x * x).sum::<f32>() / s.samples.len() as f32).sqrt()
    }

    #[test]
    fn the_first_bytes_pick_the_decoder() {
        assert_eq!(sniff(OGG), Some(Format::Ogg));
        assert_eq!(sniff(MP3), Some(Format::Mp3));
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt "), Some(Format::Wav));
        assert_eq!(sniff(b"ID3\x03"), Some(Format::Mp3));
        assert_eq!(sniff(b"PK\x03\x04"), None);
        assert!(decode_any(b"nope").unwrap_err().contains("not a WAV"));
    }

    #[test]
    fn ogg_vorbis_decodes_to_the_tone_that_was_encoded() {
        let s = decode_any(OGG).unwrap();
        assert_eq!(s.sample_rate, 22_050);
        assert!(
            (s.duration_secs() - 0.25).abs() < 0.06,
            "{}",
            s.duration_secs()
        );
        assert!((hz(&s) - 440.0).abs() < 25.0, "{} Hz", hz(&s));
        // A full-scale-ish sine of amplitude 0.125 has an RMS near 0.088.
        assert!((0.03..0.2).contains(&rms(&s)), "{}", rms(&s));
        assert!(s.samples.iter().all(|x| x.abs() <= 1.0));
    }

    #[test]
    fn stereo_ogg_is_mixed_down_to_mono_at_the_same_length() {
        let (m, st) = (decode_ogg(OGG).unwrap(), decode_ogg(OGG_STEREO).unwrap());
        assert_eq!(st.sample_rate, 22_050);
        assert!((st.samples.len() as i64 - m.samples.len() as i64).abs() < 2200);
        assert!((hz(&st) - 440.0).abs() < 25.0);
    }

    #[test]
    fn mp3_decodes_to_the_tone_that_was_encoded() {
        let s = decode_any(MP3).unwrap();
        assert_eq!(s.sample_rate, 44_100);
        assert!(
            (s.duration_secs() - 0.25).abs() < 0.08,
            "{}",
            s.duration_secs()
        );
        assert!((hz(&s) - 440.0).abs() < 30.0, "{} Hz", hz(&s));
        assert!((0.03..0.2).contains(&rms(&s)), "{}", rms(&s));
    }

    #[test]
    fn a_low_sample_rate_mp3_is_refused_not_played_at_the_wrong_pitch() {
        const LOW: &[u8] = include_bytes!("../tests/data/tone440_22k.mp3");
        assert!(decode_mp3(LOW).unwrap_err().contains("only MPEG-1"));
    }

    #[test]
    fn truncated_and_garbage_files_are_errors_not_panics() {
        assert!(decode_ogg(&OGG[..40]).is_err());
        assert!(decode_ogg(b"OggS garbage that is not a stream").is_err());
        assert!(decode_mp3(&MP3[..3]).is_err());
        assert!(decode_mp3(&[0xFF, 0xFB, 0, 0, 0, 0, 0, 0]).is_err());
    }
}
