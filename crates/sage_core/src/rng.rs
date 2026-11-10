//! The world's seeded RNG. The simulation never touches OS randomness.
//!
//! SplitMix64 for seeding and xoshiro256** for the stream: small, fast,
//! fully specified, and identical on every platform.

/// A deterministic pseudo-random generator owned by the world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn from_seed(seed: u64) -> Self {
        let mut sm = seed;
        Self {
            s: [
                splitmix64(&mut sm),
                splitmix64(&mut sm),
                splitmix64(&mut sm),
                splitmix64(&mut sm),
            ],
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in `[0, 1)` with 24 bits of precision (exact in f32).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 * (1.0 / (1u64 << 24) as f32)
    }

    /// Uniform in `[lo, hi)`.
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    pub fn next_bool(&mut self) -> bool {
        self.next_u64() >> 63 == 1
    }

    /// Internal state, for hashing and snapshots.
    pub fn state(&self) -> [u64; 4] {
        self.s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::from_seed(42);
        let mut b = Rng::from_seed(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn known_first_value() {
        // Pins the algorithm: changing it changes every recorded replay.
        let mut r = Rng::from_seed(0);
        assert_eq!(r.next_u64(), 0x99EC_5F36_CB75_F2B4);
    }

    #[test]
    fn f32_in_range() {
        let mut r = Rng::from_seed(7);
        for _ in 0..10_000 {
            let x = r.range_f32(-2.0, 3.0);
            assert!((-2.0..3.0).contains(&x));
        }
    }
}
