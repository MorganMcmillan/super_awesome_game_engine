//! Input is data: one `TickInput` per tick, recorded and replayable.
//!
//! File format (text, one tick per line, versioned header):
//! ```text
//! sage-input v1 seed=<u64>
//! <tick> <buttons as hex>
//! ```
//! Ticks with no buttons held may be omitted; missing ticks replay as empty.
//! Lines starting with `#` are comments.

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

/// A recorded run: the seed plus the input for each tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputRecording {
    pub seed: u64,
    /// Ordered map so serialization is canonical.
    ticks: BTreeMap<u64, TickInput>,
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

    pub fn get(&self, tick: u64) -> TickInput {
        self.ticks.get(&tick).copied().unwrap_or_default()
    }

    pub fn to_text(&self) -> String {
        let mut out = format!("{FORMAT_HEADER} seed={} len={}\n", self.seed, self.len);
        for (tick, input) in &self.ticks {
            out.push_str(&format!("{tick} {:x}\n", input.buttons));
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
        let rest = header
            .strip_prefix(FORMAT_HEADER)
            .ok_or_else(|| err(1, "missing 'sage-input v1' header"))?;
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
            if parts.next().is_some() {
                return Err(err(i + 1, "trailing data"));
            }
            rec.record(tick, TickInput { buttons });
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
    fn rejects_bad_header() {
        assert!(InputRecording::from_text("nope\n").is_err());
    }
}
