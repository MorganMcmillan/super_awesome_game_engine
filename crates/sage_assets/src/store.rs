//! A directory of assets with stable ids, a sorted catalog and a cache.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::{AssetId, Image, Sound, fnv1a, normalize_path};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetKind {
    Image,
    Sound,
}

impl AssetKind {
    pub fn of_path(path: &str) -> Option<AssetKind> {
        let ext = path.rsplit('.').next()?.to_ascii_lowercase();
        match ext.as_str() {
            "png" => Some(AssetKind::Image),
            "wav" | "ogg" | "mp3" => Some(AssetKind::Sound),
            _ => None,
        }
    }
}

/// One asset found under the root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogEntry {
    pub id: AssetId,
    /// Normalized path relative to the root.
    pub path: String,
    pub kind: AssetKind,
    /// FNV-1a of the file's bytes: a cache key and change detector.
    pub content_hash: u64,
}

/// Loads and caches assets from a root directory. Cached values are shared
/// (`Rc`), and a file is decoded again only when its content hash changed,
/// which is what [`AssetStore::reload`] checks.
pub struct AssetStore {
    root: PathBuf,
    images: BTreeMap<AssetId, (u64, Rc<Image>)>,
    sounds: BTreeMap<AssetId, (u64, Rc<Sound>)>,
    decodes: usize,
}

impl AssetStore {
    pub fn new(root: impl Into<PathBuf>) -> AssetStore {
        AssetStore {
            root: root.into(),
            images: BTreeMap::new(),
            sounds: BTreeMap::new(),
            decodes: 0,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// How many files have been decoded so far (cache hits do not count).
    pub fn decode_count(&self) -> usize {
        self.decodes
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        let full = self.root.join(normalize_path(path));
        std::fs::read(&full).map_err(|e| format!("{}: {e}", full.display()))
    }

    /// Lists every known asset under the root, sorted by path (so the order
    /// never depends on the file system).
    pub fn catalog(&self) -> Result<Vec<CatalogEntry>, String> {
        let mut found = vec![];
        self.scan(&self.root, "", &mut found)?;
        found.sort_by(|a: &CatalogEntry, b| a.path.cmp(&b.path));
        Ok(found)
    }

    fn scan(&self, dir: &Path, prefix: &str, out: &mut Vec<CatalogEntry>) -> Result<(), String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let file_type = entry.file_type().map_err(|e| e.to_string())?;
            if file_type.is_dir() {
                self.scan(&entry.path(), &rel, out)?;
            } else if let Some(kind) = AssetKind::of_path(&rel) {
                let bytes = std::fs::read(entry.path()).map_err(|e| e.to_string())?;
                out.push(CatalogEntry {
                    id: AssetId::of_path(&rel),
                    path: rel,
                    kind,
                    content_hash: fnv1a(&bytes),
                });
            }
        }
        Ok(())
    }

    pub fn load_image(&mut self, path: &str) -> Result<(AssetId, Rc<Image>), String> {
        let id = AssetId::of_path(path);
        let bytes = self.read(path)?;
        let hash = fnv1a(&bytes);
        if let Some((h, img)) = self.images.get(&id)
            && *h == hash
        {
            return Ok((id, img.clone()));
        }
        let img = Rc::new(Image::from_png_bytes(&bytes).map_err(|e| format!("{path}: {e}"))?);
        self.decodes += 1;
        self.images.insert(id, (hash, img.clone()));
        Ok((id, img))
    }

    pub fn load_sound(&mut self, path: &str) -> Result<(AssetId, Rc<Sound>), String> {
        let id = AssetId::of_path(path);
        let bytes = self.read(path)?;
        let hash = fnv1a(&bytes);
        if let Some((h, snd)) = self.sounds.get(&id)
            && *h == hash
        {
            return Ok((id, snd.clone()));
        }
        let snd =
            Rc::new(crate::compressed::decode_any(&bytes).map_err(|e| format!("{path}: {e}"))?);
        self.decodes += 1;
        self.sounds.insert(id, (hash, snd.clone()));
        Ok((id, snd))
    }

    /// Re-reads every cached asset and returns the ids whose content changed.
    /// Unchanged files are not decoded again.
    pub fn reload(&mut self, catalog: &[CatalogEntry]) -> Result<Vec<AssetId>, String> {
        let mut changed = vec![];
        for e in catalog {
            let cached_hash = match e.kind {
                AssetKind::Image => self.images.get(&e.id).map(|c| c.0),
                AssetKind::Sound => self.sounds.get(&e.id).map(|c| c.0),
            };
            let Some(old) = cached_hash else { continue };
            let now = fnv1a(&self.read(&e.path)?);
            if now != old {
                match e.kind {
                    AssetKind::Image => self.load_image(&e.path).map(|_| ())?,
                    AssetKind::Sound => self.load_sound(&e.path).map(|_| ())?,
                }
                changed.push(e.id);
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wav;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sage_assets_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        dir
    }

    #[test]
    fn catalog_is_sorted_and_ids_are_path_based() {
        let root = temp_root("catalog");
        let png = Image::solid(2, 2, [1, 2, 3, 255]).to_png_bytes().unwrap();
        let wav = wav::encode_pcm16(&wav::square_blip(8000, 200.0, 0.01, 0.5));
        std::fs::write(root.join("sub/b.png"), &png).unwrap();
        std::fs::write(root.join("a.wav"), &wav).unwrap();
        std::fs::write(root.join("notes.txt"), b"ignored").unwrap();
        let cat = AssetStore::new(&root).catalog().unwrap();
        let paths: Vec<_> = cat.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a.wav", "sub/b.png"]);
        assert_eq!(cat[1].id, AssetId::of_path("sub\\b.png"));
        assert_eq!(cat[0].kind, AssetKind::Sound);
        assert_eq!(cat[1].content_hash, fnv1a(&png));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cache_decodes_once_and_reload_detects_changes() {
        let root = temp_root("cache");
        let one = Image::solid(2, 2, [9, 9, 9, 255]);
        std::fs::write(root.join("x.png"), one.to_png_bytes().unwrap()).unwrap();
        let mut store = AssetStore::new(&root);
        let (id, a) = store.load_image("x.png").unwrap();
        let (_, b) = store.load_image("x.png").unwrap();
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(store.decode_count(), 1);

        let cat = store.catalog().unwrap();
        assert!(store.reload(&cat).unwrap().is_empty());
        assert_eq!(store.decode_count(), 1);

        let two = Image::solid(2, 2, [7, 7, 7, 255]);
        std::fs::write(root.join("x.png"), two.to_png_bytes().unwrap()).unwrap();
        assert_eq!(store.reload(&cat).unwrap(), vec![id]);
        assert_eq!(
            store.load_image("x.png").unwrap().1.pixel(0, 0),
            [7, 7, 7, 255]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn missing_and_corrupt_files_report_their_path() {
        let root = temp_root("errors");
        std::fs::write(root.join("bad.png"), b"nope").unwrap();
        let mut store = AssetStore::new(&root);
        assert!(store.load_image("missing.png").is_err());
        let err = store.load_image("bad.png").unwrap_err();
        assert!(err.contains("bad.png"), "{err}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ogg_and_mp3_files_are_sounds_the_store_decodes_by_content() {
        let dir = temp_root("compressed");
        let data = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
        std::fs::copy(format!("{data}tone440.ogg"), dir.join("a.ogg")).unwrap();
        std::fs::copy(format!("{data}tone440.mp3"), dir.join("b.mp3")).unwrap();
        // A file whose name lies about its content is still decoded as what
        // it is.
        std::fs::copy(format!("{data}tone440.ogg"), dir.join("c.mp3")).unwrap();
        let mut store = AssetStore::new(&dir);
        let cat = store.catalog().unwrap();
        assert!(cat.iter().all(|e| e.kind == AssetKind::Sound), "{cat:?}");
        for name in ["a.ogg", "b.mp3", "c.mp3"] {
            let (_, s) = store.load_sound(name).unwrap();
            assert!(s.duration_secs() > 0.1, "{name}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
