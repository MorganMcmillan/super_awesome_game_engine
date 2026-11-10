//! World-state hashing. FNV-1a 64 over explicitly little-endian bytes, so
//! the hash depends only on the values fed in and the order they are fed,
//! never on memory layout, pointer values or process state.
//!
//! Callers are responsible for canonical order: feed entities sorted by
//! `StableId`, never in Flecs iteration order.

#[derive(Clone, Debug)]
pub struct StateHasher {
    h: u64,
}

const OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01B3;

impl Default for StateHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl StateHasher {
    pub fn new() -> Self {
        Self { h: OFFSET }
    }

    pub fn bytes(&mut self, bytes: &[u8]) -> &mut Self {
        for b in bytes {
            self.h ^= *b as u64;
            self.h = self.h.wrapping_mul(PRIME);
        }
        self
    }

    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.bytes(&[v])
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }

    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }

    /// Hashes the exact bit pattern (so `-0.0 != 0.0` and NaN payloads count).
    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.u32(v.to_bits())
    }

    /// A tag separating sections, so `[a][bc]` and `[ab][c]` hash differently.
    pub fn tag(&mut self, name: &str) -> &mut Self {
        self.u32(name.len() as u32).bytes(name.as_bytes())
    }

    pub fn finish(&self) -> u64 {
        self.h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_reference_vector() {
        assert_eq!(
            StateHasher::new().bytes(b"a").finish(),
            0xAF63_DC4C_8601_EC8C
        );
    }

    #[test]
    fn order_matters() {
        let a = StateHasher::new().u32(1).u32(2).finish();
        let b = StateHasher::new().u32(2).u32(1).finish();
        assert_ne!(a, b);
    }

    #[test]
    fn tags_separate_sections() {
        let a = StateHasher::new().tag("ab").tag("c").finish();
        let b = StateHasher::new().tag("a").tag("bc").finish();
        assert_ne!(a, b);
    }
}
