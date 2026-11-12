//! Asset pipeline basics (Milestone 1): stable ids, decoding, a built-in
//! bitmap font and a content-hashed store.
//!
//! This crate needs no window, GPU or audio device, so every part of it is
//! testable headless. It is presentation-side: simulation crates do not
//! depend on it. A simulation that wants to name an asset stores its
//! [`AssetId`], which depends only on the asset's path.

pub mod compressed;
mod font;
mod image_asset;
pub mod proportional;
mod store;
pub mod wav;

pub use font::{BitmapFont, GlyphQuad};
pub use image_asset::Image;
pub use store::{AssetKind, AssetStore, CatalogEntry};
pub use wav::Sound;

/// A stable identifier for an asset: FNV-1a of its normalized path (forward
/// slashes, no leading `./`). The same path gives the same id on every
/// machine and run, so ids can be stored in scenes and replays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetId(pub u64);

impl AssetId {
    pub fn of_path(path: &str) -> AssetId {
        AssetId(fnv1a(normalize_path(path).as_bytes()))
    }
}

/// Normalizes a relative asset path: backslashes become slashes and leading
/// `./` is dropped.
pub fn normalize_path(path: &str) -> String {
    let p = path.replace('\\', "/");
    p.trim_start_matches("./").to_string()
}

/// 64-bit FNV-1a, used for both asset ids and content hashes.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_depend_only_on_the_normalized_path() {
        assert_eq!(AssetId::of_path("a/b.png"), AssetId::of_path("a\\b.png"));
        assert_eq!(AssetId::of_path("a/b.png"), AssetId::of_path("./a/b.png"));
        assert_ne!(AssetId::of_path("a/b.png"), AssetId::of_path("a/c.png"));
        // Pinned: ids are stored in data, so they must never change silently.
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
