//! Input mapping: physical inputs (keys, gamepad buttons and sticks) to the
//! game's input bits, with rebinding and a text form for saving bindings.
//!
//! The game owns the meaning of the bits; the mapping is sampled once per
//! tick into a `TickInput`, which is what gets recorded and replayed. A
//! rebinding therefore never affects determinism: replays contain bits, not
//! keys.

use sage_core::TickInput;
use sdl3::gamepad::{Axis, Button};
use sdl3::keyboard::Scancode;

/// A physical input that can be bound to a game input bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    Key(Scancode),
    PadButton(Button),
    /// One direction of a stick or trigger axis.
    PadAxis {
        axis: Axis,
        positive: bool,
    },
}

impl Binding {
    /// A stable text name: `key:W`, `pad:dpup`, `axis:lefty-`.
    pub fn to_text(self) -> String {
        match self {
            Binding::Key(k) => format!("key:{}", k.name()),
            Binding::PadButton(b) => format!("pad:{}", b.string()),
            Binding::PadAxis { axis, positive } => {
                format!("axis:{}{}", axis.string(), if positive { '+' } else { '-' })
            }
        }
    }

    pub fn from_text(text: &str) -> Result<Binding, String> {
        let bad = || format!("unknown binding '{text}'");
        let (kind, name) = text.split_once(':').ok_or_else(bad)?;
        match kind {
            "key" => Scancode::from_name(name).map(Binding::Key).ok_or_else(bad),
            "pad" => Button::from_string(name)
                .map(Binding::PadButton)
                .ok_or_else(bad),
            "axis" => {
                let (axis, sign) = name.split_at(name.len().checked_sub(1).ok_or_else(bad)?);
                let positive = match sign {
                    "+" => true,
                    "-" => false,
                    _ => return Err(bad()),
                };
                Axis::from_string(axis)
                    .map(|axis| Binding::PadAxis { axis, positive })
                    .ok_or_else(bad)
            }
            _ => Err(bad()),
        }
    }
}

/// What the mapping reads. The window loop implements it over SDL's keyboard
/// and gamepads; tests implement it with fixed values.
pub trait InputSource {
    fn key(&self, key: Scancode) -> bool;
    fn pad_button(&self, button: Button) -> bool;
    /// Axis position in [-1, 1].
    fn pad_axis(&self, axis: Axis) -> f32;
}

/// How far a stick must move before it counts as pressed.
pub const AXIS_THRESHOLD: f32 = 0.5;

#[derive(Clone, Debug, Default)]
pub struct InputMap {
    /// (physical input, game bit). Several inputs may map to one bit and
    /// one input may drive several bits.
    pub bindings: Vec<(Binding, u32)>,
}

impl InputMap {
    /// Binds a key. Kept as the simple form for games that only use keys.
    pub fn bind(self, key: Scancode, bit: u32) -> Self {
        self.bind_input(Binding::Key(key), bit)
    }

    pub fn bind_input(mut self, binding: Binding, bit: u32) -> Self {
        if !self.bindings.contains(&(binding, bit)) {
            self.bindings.push((binding, bit));
        }
        self
    }

    pub fn bindings_for(&self, bit: u32) -> Vec<Binding> {
        self.bindings
            .iter()
            .filter(|b| b.1 == bit)
            .map(|b| b.0)
            .collect()
    }

    /// Replaces `old` with `new` on `bit`. False when `old` was not bound there.
    pub fn rebind(&mut self, bit: u32, old: Binding, new: Binding) -> bool {
        match self.bindings.iter_mut().find(|b| **b == (old, bit)) {
            Some(slot) => {
                slot.0 = new;
                self.bindings.dedup();
                true
            }
            None => false,
        }
    }

    /// Removes every binding of `bit`.
    pub fn unbind(&mut self, bit: u32) {
        self.bindings.retain(|b| b.1 != bit);
    }

    pub fn sample(&self, source: &dyn InputSource) -> TickInput {
        self.bindings
            .iter()
            .fold(TickInput::EMPTY, |acc, &(binding, bit)| {
                let held = match binding {
                    Binding::Key(k) => source.key(k),
                    Binding::PadButton(b) => source.pad_button(b),
                    Binding::PadAxis { axis, positive } => {
                        let v = source.pad_axis(axis);
                        if positive {
                            v > AXIS_THRESHOLD
                        } else {
                            v < -AXIS_THRESHOLD
                        }
                    }
                };
                // OR, not assignment: a second binding must not release a bit
                // another binding is holding.
                if held { acc.with(bit, true) } else { acc }
            })
    }

    /// `sage-bindings v1`, then one `<bit> <binding>` line per binding.
    pub fn to_text(&self) -> String {
        let mut s = String::from("sage-bindings v1\n");
        for (binding, bit) in &self.bindings {
            s.push_str(&format!("{bit} {}\n", binding.to_text()));
        }
        s
    }

    pub fn from_text(text: &str) -> Result<InputMap, String> {
        let mut lines = text.lines();
        if lines.next() != Some("sage-bindings v1") {
            return Err("not a sage-bindings v1 file".into());
        }
        let mut map = InputMap::default();
        for line in lines.filter(|l| !l.trim().is_empty()) {
            let (bit, binding) = line
                .split_once(' ')
                .ok_or_else(|| format!("bad line '{line}'"))?;
            let bit: u32 = bit.parse().map_err(|_| format!("bad bit in '{line}'"))?;
            map = map.bind_input(Binding::from_text(binding)?, bit);
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        keys: Vec<Scancode>,
        buttons: Vec<Button>,
        axes: Vec<(Axis, f32)>,
    }

    impl InputSource for Fake {
        fn key(&self, key: Scancode) -> bool {
            self.keys.contains(&key)
        }
        fn pad_button(&self, button: Button) -> bool {
            self.buttons.contains(&button)
        }
        fn pad_axis(&self, axis: Axis) -> f32 {
            self.axes.iter().find(|a| a.0 == axis).map_or(0.0, |a| a.1)
        }
    }

    fn map() -> InputMap {
        InputMap::default()
            .bind(Scancode::W, 1)
            .bind(Scancode::S, 2)
            .bind_input(Binding::PadButton(Button::DPadUp), 1)
            .bind_input(
                Binding::PadAxis {
                    axis: Axis::LeftY,
                    positive: false,
                },
                1,
            )
            .bind_input(
                Binding::PadAxis {
                    axis: Axis::LeftY,
                    positive: true,
                },
                2,
            )
    }

    #[test]
    fn keys_set_bits() {
        let m = map();
        let keys = |k: &[Scancode]| Fake {
            keys: k.to_vec(),
            ..Default::default()
        };
        assert_eq!(m.sample(&keys(&[Scancode::S])).buttons, 2);
        assert_eq!(m.sample(&keys(&[Scancode::W, Scancode::S])).buttons, 3);
        assert_eq!(m.sample(&keys(&[])), TickInput::EMPTY);
    }

    #[test]
    fn gamepad_buttons_and_sticks_drive_the_same_bits() {
        let m = map();
        let pad = Fake {
            buttons: vec![Button::DPadUp],
            ..Default::default()
        };
        assert_eq!(m.sample(&pad).buttons, 1);
        let stick_down = Fake {
            axes: vec![(Axis::LeftY, 0.9)],
            ..Default::default()
        };
        assert_eq!(m.sample(&stick_down).buttons, 2);
        let stick_up = Fake {
            axes: vec![(Axis::LeftY, -0.9)],
            ..Default::default()
        };
        assert_eq!(m.sample(&stick_up).buttons, 1);
        let drift = Fake {
            axes: vec![(Axis::LeftY, 0.2)],
            ..Default::default()
        };
        assert_eq!(m.sample(&drift), TickInput::EMPTY);
    }

    #[test]
    fn a_released_binding_does_not_cancel_a_held_one() {
        // W is up while the d-pad up is held: bit 1 stays set.
        let m = map();
        let src = Fake {
            buttons: vec![Button::DPadUp],
            ..Default::default()
        };
        assert_eq!(m.sample(&src).buttons & 1, 1);
    }

    #[test]
    fn rebinding_moves_a_bit_to_another_key() {
        let mut m = map();
        assert!(m.rebind(1, Binding::Key(Scancode::W), Binding::Key(Scancode::Up)));
        assert!(!m.rebind(1, Binding::Key(Scancode::W), Binding::Key(Scancode::Down)));
        let w = Fake {
            keys: vec![Scancode::W],
            ..Default::default()
        };
        let up = Fake {
            keys: vec![Scancode::Up],
            ..Default::default()
        };
        assert_eq!(m.sample(&w), TickInput::EMPTY);
        assert_eq!(m.sample(&up).buttons, 1);
        m.unbind(1);
        assert_eq!(m.sample(&up), TickInput::EMPTY);
        assert!(m.bindings_for(1).is_empty());
        assert_eq!(m.bindings_for(2).len(), 2);
    }

    #[test]
    fn bindings_round_trip_through_text() {
        let m = map();
        let back = InputMap::from_text(&m.to_text()).unwrap();
        assert_eq!(back.bindings, m.bindings);
        assert!(m.to_text().starts_with("sage-bindings v1\n1 key:W\n"));
        assert!(InputMap::from_text("nope").is_err());
        assert!(InputMap::from_text("sage-bindings v1\n1 key:NotAKey\n").is_err());
        assert!(InputMap::from_text("sage-bindings v1\nx key:W\n").is_err());
        assert!(Binding::from_text("axis:lefty*").is_err());
        assert!(Binding::from_text("axis:").is_err());
    }
}
