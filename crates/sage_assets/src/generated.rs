//! Assets generated in code and committed as files.
//!
//! The examples draw their placeholder art and sounds in code, so nothing is
//! downloaded or licensed, and commit the output so a game loads it like any
//! other asset. A generator is a list of `(file name, bytes)`; this writes it
//! to a directory and checks that the committed files still match it.

use std::path::Path;

/// Writes every generated file into `dir` (created if missing). Returns one
/// line per file, for a `gen-assets` command to print.
pub fn write_all<N: AsRef<str>>(
    dir: impl AsRef<Path>,
    files: &[(N, Vec<u8>)],
) -> Result<Vec<String>, String> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    files
        .iter()
        .map(|(name, bytes)| {
            let path = dir.join(name.as_ref());
            std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(format!("{}: {} bytes", name.as_ref(), bytes.len()))
        })
        .collect()
}

/// The generated files whose committed copy in `dir` is missing or differs
/// (empty when the directory is up to date).
pub fn stale<N: AsRef<str>>(dir: impl AsRef<Path>, files: &[(N, Vec<u8>)]) -> Vec<String> {
    files
        .iter()
        .filter(|(name, bytes)| {
            std::fs::read(dir.as_ref().join(name.as_ref())).ok().as_ref() != Some(bytes)
        })
        .map(|(name, _)| name.as_ref().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_files_are_not_stale_and_a_changed_generator_is() {
        let dir = std::env::temp_dir().join(format!("sage-generated-{}", std::process::id()));
        let files = vec![("a.bin", vec![1u8, 2]), ("b.bin", vec![3u8])];
        assert_eq!(stale(&dir, &files), ["a.bin", "b.bin"]);
        let lines = write_all(&dir, &files).unwrap();
        assert_eq!(lines, ["a.bin: 2 bytes", "b.bin: 1 bytes"]);
        assert!(stale(&dir, &files).is_empty());
        let changed = vec![("a.bin", vec![1u8, 2]), ("b.bin", vec![4u8])];
        assert_eq!(stale(&dir, &changed), ["b.bin"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
