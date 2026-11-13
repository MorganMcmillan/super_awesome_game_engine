//! The block editor's round trip, palette and notes, in the embedded
//! interpreter (`python/tests/test_blocks.py`).

#[test]
fn the_block_model_passes_its_python_tests() {
    let failures = sage_editor::blocks::run_python_tests(
        "test_blocks",
        include_str!("../../../python/tests/test_blocks.py"),
    )
    .unwrap();
    assert!(failures.is_empty(), "{failures}");
}
