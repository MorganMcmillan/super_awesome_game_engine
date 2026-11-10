//! The editor stubs for Pong's native vocabulary (`python/sage/*.pyi`, the
//! generated ones) must match what the binding generates from reflection.
//!
//! After an intentional change to a native component, regenerate them with
//! `UPDATE_STUBS=1 cargo test -p pong_py --test stubs`.

use std::path::PathBuf;

use pong_py::{PongPy, PongPyConfig};

#[test]
fn committed_type_stubs_match_the_native_components() {
    let p = PongPy::new(PongPyConfig::new(1)).unwrap();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python/sage");
    let update = std::env::var_os("UPDATE_STUBS").is_some();
    let stubs = p.ecs().type_stubs();
    assert!(!stubs.is_empty());
    let mut stale = vec![];
    for (file, text) in stubs {
        let path = dir.join(&file);
        if update {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, &text).unwrap();
            continue;
        }
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        if committed != text {
            stale.push(file);
        }
    }
    assert!(
        stale.is_empty(),
        "stale editor stubs in python/sage: {stale:?}; \
         regenerate with `UPDATE_STUBS=1 cargo test -p pong_py --test stubs`"
    );
}
