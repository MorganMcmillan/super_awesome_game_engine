//! The determinism harness (spike S3, acceptance A1).
//!
//! Any simulation that consumes one `TickInput` per tick and can hash its
//! state can be replayed twice and compared tick by tick.

use crate::input::{InputRecording, TickInput};

/// A deterministic, headless simulation.
pub trait Simulation {
    /// Advances exactly one fixed timestep using `input`.
    fn step(&mut self, input: TickInput);
    /// The number of completed ticks.
    fn tick(&self) -> u64;
    /// A hash of the complete simulation state, fed in canonical order.
    fn state_hash(&self) -> u64;
}

/// Per-tick hashes of one run. `hashes[i]` is the state after tick `i + 1`;
/// `initial` is the state before the first tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunTrace {
    pub initial: u64,
    pub hashes: Vec<u64>,
}

/// Runs `sim` over every tick of `recording` and records the hash after each.
pub fn run_recording<S: Simulation>(sim: &mut S, recording: &InputRecording) -> RunTrace {
    let initial = sim.state_hash();
    let mut hashes = Vec::with_capacity(recording.len as usize);
    for tick in 0..recording.len {
        sim.step(recording.get(tick));
        hashes.push(sim.state_hash());
    }
    RunTrace { initial, hashes }
}

/// Where two runs first differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Divergence {
    /// `None` when the initial states differ, else the 0-based tick index
    /// whose resulting state differs.
    pub tick: Option<u64>,
    pub left: u64,
    pub right: u64,
}

pub fn first_divergence(a: &RunTrace, b: &RunTrace) -> Option<Divergence> {
    if a.initial != b.initial {
        return Some(Divergence {
            tick: None,
            left: a.initial,
            right: b.initial,
        });
    }
    for (i, (x, y)) in a.hashes.iter().zip(&b.hashes).enumerate() {
        if x != y {
            return Some(Divergence {
                tick: Some(i as u64),
                left: *x,
                right: *y,
            });
        }
    }
    if a.hashes.len() != b.hashes.len() {
        let n = a.hashes.len().min(b.hashes.len());
        return Some(Divergence {
            tick: Some(n as u64),
            left: a.hashes.get(n).copied().unwrap_or(0),
            right: b.hashes.get(n).copied().unwrap_or(0),
        });
    }
    None
}

/// Builds two fresh simulations with `make`, replays `recording` on both and
/// returns the first divergence, if any.
pub fn replay_twice<S, F>(mut make: F, recording: &InputRecording) -> Option<Divergence>
where
    S: Simulation,
    F: FnMut(u64) -> S,
{
    let a = run_recording(&mut make(recording.seed), recording);
    let b = run_recording(&mut make(recording.seed), recording);
    first_divergence(&a, &b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::StateHasher;
    use std::collections::HashMap;

    struct Counter {
        tick: u64,
        total: u64,
        /// Canary: when set, the step iterates a HashMap in simulation code.
        canary: bool,
        map: HashMap<u32, u64>,
    }

    impl Counter {
        fn new(canary: bool) -> Self {
            let mut map = HashMap::new();
            for k in 0..64 {
                map.insert(k, k as u64);
            }
            Self {
                tick: 0,
                total: 0,
                canary,
                map,
            }
        }
    }

    impl Simulation for Counter {
        fn step(&mut self, input: TickInput) {
            self.tick += 1;
            self.total = self
                .total
                .wrapping_mul(31)
                .wrapping_add(input.buttons as u64);
            if self.canary {
                // Order-dependent fold over a randomly seeded HashMap.
                for v in self.map.values() {
                    self.total = self.total.wrapping_mul(131).wrapping_add(*v);
                }
            }
        }
        fn tick(&self) -> u64 {
            self.tick
        }
        fn state_hash(&self) -> u64 {
            StateHasher::new().u64(self.tick).u64(self.total).finish()
        }
    }

    fn recording() -> InputRecording {
        let mut r = InputRecording::new(1);
        for t in 0..50 {
            r.record(
                t,
                TickInput {
                    buttons: (t % 3) as u32,
                },
            );
        }
        r
    }

    #[test]
    fn deterministic_sim_matches() {
        assert_eq!(replay_twice(|_| Counter::new(false), &recording()), None);
    }

    #[test]
    fn hashmap_canary_is_caught() {
        let d = replay_twice(|_| Counter::new(true), &recording());
        assert_eq!(d.map(|d| d.tick), Some(Some(0)));
    }
}
