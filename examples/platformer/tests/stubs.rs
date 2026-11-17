//! The editor stubs for the kit's native vocabulary (`python/sage/*.pyi`, the
//! generated ones) must match what the binding generates from reflection.
//!
//! After an intentional change to a native component, regenerate them with
//! `UPDATE_STUBS=1 cargo test -p platformer --test stubs`.

use std::path::PathBuf;

use platformer::config;
use sage_platformer::Platformer;

#[test]
fn committed_type_stubs_match_the_native_components() {
    let p = Platformer::new(config(1)).unwrap();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python/sage");
    let update = std::env::var_os("UPDATE_STUBS").is_some();
    let stubs = p.ecs().type_stubs();
    assert!(
        stubs.iter().any(|s| s.0 == "platformer.pyi"),
        "{:?}",
        stubs.iter().map(|s| &s.0).collect::<Vec<_>>()
    );
    let mut stale = vec![];
    for (file, text) in stubs {
        let path = dir.join(&file);
        if update {
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
         regenerate with `UPDATE_STUBS=1 cargo test -p platformer --test stubs`"
    );
}
