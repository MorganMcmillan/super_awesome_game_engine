//! The editor stub for `sage.ui` (`python/sage/ui.pyi`) is generated from the
//! reflection data of the UI components. After a change to one, regenerate it
//! with `UPDATE_STUBS=1 cargo test -p sage_ui --test stubs`.
//!
//! Only `ui.pyi` is checked: the generator also emits `_native.pyi` for the
//! parent namespace, which belongs to whichever host owns the components
//! directly under `sage` (Pong's `Input` and `Clock`).

use std::path::PathBuf;

use flecs_ecs::prelude::*;
use sage_ecs_py::Ecs;

#[test]
fn committed_ui_stub_matches_the_components() {
    let world = World::new();
    let ecs = Ecs::new(&world, 1, "game").unwrap();
    sage_ui::setup(&world, &ecs).unwrap();
    let (_, text) = ecs
        .type_stubs()
        .into_iter()
        .find(|(file, _)| file == "ui.pyi")
        .expect("the UI components are importable from sage.ui");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python/sage/ui.pyi");
    if std::env::var_os("UPDATE_STUBS").is_some() {
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert_eq!(
        committed, text,
        "stale python/sage/ui.pyi; regenerate with `UPDATE_STUBS=1 cargo test -p sage_ui --test stubs`"
    );
}
