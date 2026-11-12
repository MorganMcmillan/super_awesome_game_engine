//! Input is data: one `TickInput` per tick, recorded and replayable.
//!
//! File format (text, one tick per line, versioned header):
//! ```text
//! sage-input v1 seed=<u64> len=<n>
//! <tick> <buttons as hex>
//! ```
//! Ticks with no buttons held may be omitted; missing ticks replay as empty.
//! Lines starting with `#` are comments.
//!
//! Version 2 adds the pointer stream a UI session needs. It is written only
//! when a recording has pointer data (otherwise the file is v1, byte for
//! byte), and both versions are read:
//! ```text
//! sage-input v2 seed=<u64> len=<n>
//! <tick> <buttons as hex> [p=<x>,<y>] [m=<hex>] [w=<wheel>] [c=<key hex>:<mods hex>] [t=<hex>]
//! ```
//! `buttons` are the mouse buttons (1 primary, 2 secondary, 4 middle); `p` is
//! the pointer in UI units (a multiple of 1/16, so it round-trips exactly);
//! `m` the held keyboard bits (the UI's `NAV_*`, which include Shift, Ctrl
//! and Alt); `w` the wheel in notches (a multiple of 1/8); `c` a key chord;
//! `t` a typed character's code point. Fields are optional and omitted when
//! empty, and as in v1 a tick with no line is empty (nothing is carried over
//! from the previous tick).

use std::collections::BTreeMap;
use std::fmt;

/// Buttons held during one tick, as a bit set. Games assign meaning to bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TickInput {
    pub buttons: u32,
}

impl TickInput {
    pub const EMPTY: TickInput = TickInput { buttons: 0 };

    pub fn held(self, button: u32) -> bool {
        self.buttons & button != 0
    }

    pub fn with(mut self, button: u32, down: bool) -> Self {
        if down {
            self.buttons |= button;
        } else {
            self.buttons &= !button;
        }
        self
    }
}

pub const FORMAT_HEADER: &str = "sage-input v1";
pub const FORMAT_HEADER_V2: &str = "sage-input v2";

/// The pointer, keyboard and typing of one tick of a UI session, in fixed
/// point so it can be compared, hashed and written exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PointerInput {
    /// The pointer in 1/16 UI units, or `None` when it is outside the window.
    pub pos: Option<[i32; 2]>,
    /// Held keyboard bits (`NAV_*`).
    pub mods: u32,
    /// Wheel notches in 1/8.
    pub wheel: i32,
    /// A key chord: key and `CHORD_*` modifiers.
    pub chord: Option<[u32; 2]>,
    /// A typed character's code point.
    pub ch: Option<u32>,
}

impl PointerInput {
    pub const EMPTY: PointerInput = PointerInput {
        pos: None,
        mods: 0,
        wheel: 0,
        chord: None,
        ch: None,
    };

    /// From the units a window gives: UI units and notches.
    pub fn new(
        pos: Option<[f32; 2]>,
        mods: u32,
        wheel: f32,
        chord: Option<[u32; 2]>,
        ch: Option<char>,
    ) -> Self {
        PointerInput {
            pos: pos.map(|p| p.map(|v| (v * 16.0).round() as i32)),
            mods,
            wheel: (wheel * 8.0).round() as i32,
            chord,
            ch: ch.map(|c| c as u32),
        }
    }

    pub fn pointer(&self) -> Option<[f32; 2]> {
        self.pos.map(|p| p.map(|v| v as f32 / 16.0))
    }

    pub fn wheel_notches(&self) -> f32 {
        self.wheel as f32 / 8.0
    }

    pub fn typed(&self) -> Option<char> {
        self.ch.and_then(char::from_u32)
    }
}

/// `v / scale` written as a plain decimal with no trailing zeros.
fn fixed_text(v: i32, scale: i32) -> String {
    let (sign, a) = if v < 0 {
        ("-", -(v as i64))
    } else {
        ("", v as i64)
    };
    let (whole, frac) = (a / scale as i64, a % scale as i64);
    if frac == 0 {
        return format!("{sign}{whole}");
    }
    // 1/16 needs four decimals and 1/8 three; trim the zeros that pad them.
    let digits = if scale == 16 { 4 } else { 3 };
    let text = format!(
        "{:0digits$}",
        frac * 10_i64.pow(digits as u32) / scale as i64
    );
    format!("{sign}{whole}.{}", text.trim_end_matches('0'))
}

fn fixed_parse(text: &str, scale: i32) -> Option<i32> {
    let v: f64 = text.parse().ok()?;
    let fixed = (v * scale as f64).round();
    // It must be exactly a multiple of 1/scale, so a typo is not rounded away.
    ((fixed / scale as f64 - v).abs() < 1e-9 && fixed.abs() < i32::MAX as f64)
        .then_some(fixed as i32)
}

/// A recorded run: the seed plus the input for each tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputRecording {
    pub seed: u64,
    /// Ordered map so serialization is canonical.
    ticks: BTreeMap<u64, TickInput>,
    /// The pointer stream of a UI session (empty for a game recording).
    pointer: BTreeMap<u64, PointerInput>,
    /// Number of ticks the recording covers (one past the last tick).
    pub len: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "input file line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

impl InputRecording {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            ticks: BTreeMap::new(),
            pointer: BTreeMap::new(),
            len: 0,
        }
    }

    pub fn record(&mut self, tick: u64, input: TickInput) {
        if input != TickInput::EMPTY {
            self.ticks.insert(tick, input);
        } else {
            self.ticks.remove(&tick);
        }
        self.len = self.len.max(tick + 1);
    }

    /// Records the pointer stream's input for a tick (see [`PointerInput`]).
    pub fn record_pointer(&mut self, tick: u64, input: PointerInput) {
        if input != PointerInput::EMPTY {
            self.pointer.insert(tick, input);
        } else {
            self.pointer.remove(&tick);
        }
        self.len = self.len.max(tick + 1);
    }

    pub fn get_pointer(&self, tick: u64) -> PointerInput {
        self.pointer.get(&tick).copied().unwrap_or_default()
    }

    /// True if any tick has pointer data (the file is then written as v2).
    pub fn has_pointer(&self) -> bool {
        !self.pointer.is_empty()
    }

    pub fn get(&self, tick: u64) -> TickInput {
        self.ticks.get(&tick).copied().unwrap_or_default()
    }

    pub fn to_text(&self) -> String {
        let header = if self.has_pointer() {
            FORMAT_HEADER_V2
        } else {
            FORMAT_HEADER
        };
        let mut out = format!("{header} seed={} len={}\n", self.seed, self.len);
        let mut at: Vec<u64> = self
            .ticks
            .keys()
            .chain(self.pointer.keys())
            .copied()
            .collect();
        at.sort_unstable();
        at.dedup();
        for tick in at {
            out.push_str(&format!("{tick} {:x}", self.get(tick).buttons));
            let p = self.get_pointer(tick);
            if let Some([x, y]) = p.pos {
                out.push_str(&format!(" p={},{}", fixed_text(x, 16), fixed_text(y, 16)));
            }
            if p.mods != 0 {
                out.push_str(&format!(" m={:x}", p.mods));
            }
            if p.wheel != 0 {
                out.push_str(&format!(" w={}", fixed_text(p.wheel, 8)));
            }
            if let Some([key, mods]) = p.chord {
                out.push_str(&format!(" c={key:x}:{mods:x}"));
            }
            if let Some(ch) = p.ch {
                out.push_str(&format!(" t={ch:x}"));
            }
            out.push('\n');
        }
        out
    }

    pub fn from_text(text: &str) -> Result<Self, ParseError> {
        let err = |line: usize, message: &str| ParseError {
            line,
            message: message.to_string(),
        };
        let mut lines = text.lines().enumerate();
        let (_, header) = lines.next().ok_or_else(|| err(1, "empty file"))?;
        let (rest, v2) = if let Some(rest) = header.strip_prefix(FORMAT_HEADER_V2) {
            (rest, true)
        } else {
            let rest = header
                .strip_prefix(FORMAT_HEADER)
                .ok_or_else(|| err(1, "missing 'sage-input v1' or 'v2' header"))?;
            (rest, false)
        };
        let mut seed = None;
        let mut len = 0;
        for field in rest.split_whitespace() {
            if let Some(v) = field.strip_prefix("seed=") {
                seed = Some(v.parse().map_err(|_| err(1, "bad seed"))?);
            } else if let Some(v) = field.strip_prefix("len=") {
                len = v.parse().map_err(|_| err(1, "bad len"))?;
            } else {
                return Err(err(1, &format!("unknown header field '{field}'")));
            }
        }
        let mut rec = Self::new(seed.ok_or_else(|| err(1, "missing seed"))?);
        for (i, line) in lines {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let tick: u64 = parts
                .next()
                .and_then(|t| t.parse().ok())
                .ok_or_else(|| err(i + 1, "bad tick"))?;
            let buttons = parts
                .next()
                .and_then(|b| u32::from_str_radix(b, 16).ok())
                .ok_or_else(|| err(i + 1, "bad buttons"))?;
            let mut p = PointerInput::EMPTY;
            for field in parts {
                let bad = |what: &str| err(i + 1, &format!("bad {what}"));
                if !v2 {
                    return Err(err(i + 1, "trailing data"));
                }
                if let Some(v) = field.strip_prefix("p=") {
                    let (x, y) = v.split_once(',').ok_or_else(|| bad("pointer"))?;
                    p.pos = Some([
                        fixed_parse(x, 16).ok_or_else(|| bad("pointer"))?,
                        fixed_parse(y, 16).ok_or_else(|| bad("pointer"))?,
                    ]);
                } else if let Some(v) = field.strip_prefix("m=") {
                    p.mods = u32::from_str_radix(v, 16).map_err(|_| bad("keys"))?;
                } else if let Some(v) = field.strip_prefix("w=") {
                    p.wheel = fixed_parse(v, 8).ok_or_else(|| bad("wheel"))?;
                } else if let Some(v) = field.strip_prefix("c=") {
                    let (k, m) = v.split_once(':').ok_or_else(|| bad("chord"))?;
                    p.chord = Some([
                        u32::from_str_radix(k, 16).map_err(|_| bad("chord"))?,
                        u32::from_str_radix(m, 16).map_err(|_| bad("chord"))?,
                    ]);
                } else if let Some(v) = field.strip_prefix("t=") {
                    p.ch = Some(u32::from_str_radix(v, 16).map_err(|_| bad("typed character"))?);
                } else {
                    return Err(err(i + 1, &format!("unknown field '{field}'")));
                }
            }
            rec.record(tick, TickInput { buttons });
            rec.record_pointer(tick, p);
        }
        rec.len = rec.len.max(len);
        Ok(rec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut r = InputRecording::new(99);
        r.record(0, TickInput { buttons: 1 });
        r.record(5, TickInput { buttons: 0b1010 });
        r.record(9, TickInput::EMPTY);
        let text = r.to_text();
        assert_eq!(InputRecording::from_text(&text).unwrap(), r);
        assert_eq!(r.len, 10);
        assert_eq!(r.get(3), TickInput::EMPTY);
    }

    #[test]
    fn a_recording_without_pointer_data_is_written_as_v1_unchanged() {
        let mut r = InputRecording::new(7);
        r.record(2, TickInput { buttons: 3 });
        let text = r.to_text();
        assert_eq!(text, "sage-input v1 seed=7 len=3\n2 3\n");
        assert!(!r.has_pointer());
    }

    #[test]
    fn pointer_data_round_trips_as_v2_and_v1_files_still_load() {
        let mut r = InputRecording::new(5);
        r.record(0, TickInput { buttons: 1 });
        let at = PointerInput::new(Some([12.5, 3.0625]), 0x4000, 0.0, None, None);
        r.record_pointer(0, at);
        r.record(1, TickInput { buttons: 1 });
        r.record_pointer(
            1,
            PointerInput::new(Some([-1.0, 270.0]), 0, -0.375, Some([83, 1]), Some('é')),
        );
        // A tick with only the wheel, and one that is just a chord.
        r.record_pointer(4, PointerInput::new(None, 0, 1.0, None, None));
        r.record_pointer(6, PointerInput::new(None, 0, 0.0, Some([0x101, 5]), None));
        let text = r.to_text();
        assert!(text.starts_with("sage-input v2 seed=5 len=7\n"), "{text}");
        assert!(text.contains("0 1 p=12.5,3.0625 m=4000\n"), "{text}");
        assert!(
            text.contains("1 1 p=-1,270 w=-0.375 c=53:1 t=e9\n"),
            "{text}"
        );
        assert!(
            text.contains("4 0 w=1\n") && text.contains("6 0 c=101:5\n"),
            "{text}"
        );
        let back = InputRecording::from_text(&text).unwrap();
        assert_eq!(back, r);
        assert_eq!(back.get_pointer(0).pointer(), Some([12.5, 3.0625]));
        assert_eq!(back.get_pointer(1).wheel_notches(), -0.375);
        assert_eq!(back.get_pointer(1).typed(), Some('é'));
        assert_eq!(back.get_pointer(3), PointerInput::EMPTY);
        assert_eq!(back.to_text(), text, "writing it again is byte-identical");
        // A v1 file loads as before, and extra fields are errors there.
        assert_eq!(
            InputRecording::from_text("sage-input v1 seed=1 len=2\n1 2\n")
                .unwrap()
                .get(1)
                .buttons,
            2
        );
        assert!(InputRecording::from_text("sage-input v1 seed=1\n1 2 p=1,1\n").is_err());
    }

    #[test]
    fn v2_refuses_garbage_and_positions_that_are_not_sixteenths() {
        for bad in ["p=1", "p=1.03,2", "w=0.1", "c=zz:1", "m=xyz", "t=-1", "q=1"] {
            let text = format!("sage-input v2 seed=1\n0 0 {bad}\n");
            assert!(InputRecording::from_text(&text).is_err(), "{bad}");
        }
    }

    #[test]
    fn rejects_bad_header() {
        assert!(InputRecording::from_text("nope\n").is_err());
    }
}
