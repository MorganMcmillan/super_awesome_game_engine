# Milestone 2: status

Date: 2026-11-11. Scope: design doc 13.3, "UI module, scene format with migrations, save games".

M2 was started at Morgan's request (the `/goal` to implement up to M2) while Milestone 0 is open, after M1. The design doc gives M2 no acceptance list, so the checks below are mine, the way M1's were. Please confirm or replace them.

No external dependency was added: `sage_scene` has none, and `sage_ui` uses `flecs_ecs` and the internal crates.

## Decisions made during M2
Confirmed by Morgan on 2026-11-11 (prefab override diffs, item 3, are deferred to later).
1. **Three new text formats** (design doc 10.4): `sage-scene v1`, `sage-save v1` and `sage-migrations v1`. They are new, not a change to `sage-input v1`. Layout is in `crates/sage_scene/src/text.rs` and `migrate.rs`.
2. **UI hierarchy uses `ChildOf`** (design doc 9.1, an [Open] item). `Parent` does not exist at the pinned Flecs (spike S1), which was option (c) of the options listed there. Siblings flow in StableId (creation) order. `OrderedChildren` is not used, so a child cannot be reordered in place.
3. **Scenes store full component values**, not sparse prefab override diffs (10.4 says diffs). Diffs need a defined notion of "unchanged from the prefab", which is a bigger design question than M2.
4. **Strings are interned by content hash** (a 64-bit FNV-1a of the text; the empty string is 0). Components that hold text store the id; the table lives in the world, is hashed, and is saved with scenes and save games. Python reaches it with `world.intern` and `world.string`.

## Checks

| | Claim | Status | Checked by |
|---|---|---|---|
| C1 | A scene writes deterministic text that parses back to an equal scene, floats included (non-finite ones by bit pattern); the format has a golden test; parse errors name the line | **Pass** | `crates/sage_scene` tests |
| C2 | Migrations (rename field, add field with default, drop field, convert field, rename component) bring an old scene to the current schema; their text form round-trips; failures name the component, entity and step | **Pass** | `crates/sage_scene` tests; `crates/sage_ecs_py/tests/binding.rs` |
| C3 | A scene loads into a live world; a hand edit takes effect; an old-version file loads after its migration is registered; without it, the mismatch is an error (strict) or repaired with warnings (lenient); an unknown component is always an error | **Pass** | `binding.rs` |
| C4 | A save game restores the world: the hash right after loading equals the hash at the save, and every later tick's hash matches the uninterrupted run, in a Python game and in Pong at ticks 0, 1, 59, 60, 61, 200, mid-game and the last; saving the loaded world gives the same text | **Pass** | `binding.rs`, `examples/pong/tests/save.rs` |
| C5 | Pong's save carries the contact table (the previous tick's set decides what is a new begin); a save during a contact keeps it; damaged saves are errors | **Pass** | `examples/pong/tests/save.rs`; `sage_collision` tests |
| C6 | Strings are content-addressed, independent of interning order, hashed (a world without text hashes as before) and saved | **Pass** | `sage_scene` and `binding.rs` tests |
| C7 | UI layout: columns and rows, gaps, padding, `Fill` weights, alignment and justification, absolute children, whole-unit edges that neighbors share, text-sized leaves, draw order and z, malformed trees cannot loop | **Pass** | `crates/sage_ui/src/layout.rs` tests |
| C8 | UI interaction: hit testing (decoration is transparent, blocks and disabled widgets stop the pointer), click needs press and release on the same button, toggles, sliders that snap and report changes only, focus and tab order, keyboard navigation, text input, stale ids forgotten | **Pass** | `crates/sage_ui/src/interact.rs` tests |
| C9 | The UI works as entities, written in Python: layout lands in `Rect` components, widgets emit events that Python observers receive, state lives in components, two runs hash identically, a save game taken mid-press restores every later hash, disabling the `ui` flecs module pauses it | **Pass** | `crates/sage_ui/tests/ui.rs` |
| C10 | Presentation: UI frames compose to a sprite batch at a whole-number scale, centered, in order; the UI's font metrics equal the built-in font's; the demo menu draws the slider fill, toggle mark, hover tint, border and typed text where the widgets are, and the whole menu fits its screen | **Pass** | `sage_platform` tests, `examples/ui_demo/tests/render.rs` |
| C11 | The generated `sage.ui` editor stub matches the components | **Pass** | `crates/sage_ui/tests/stubs.rs` |

M0's checks and M1's still pass: `cargo test --workspace` and `--release` (S8 parity), `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` are clean on Linux (this session). They have not been run on Windows or on hosted CI for the M2 commits.

## What was built
- `crates/sage_scene` (no dependencies): `Scene`, `Save`, `Schemas`, `Migrations`, `StringTable`, the three text formats, and `Policy::Strict | Lenient` for loading.
- `crates/sage_ecs_py`: `Ecs::save_scene`, `load_scene`, `save_game`, `load_game`, `schemas`, `add_migrations`, `strings`, `intern`, `field_u64`; `Snapshot` carries the string table; Python gets `world.intern`, `world.string` and `world.migrations`. `sage_core::Rng::from_state` restores a saved generator.
- `crates/sage_ui`: pure `layout`, `interact` and `draw` modules, and the `ui` flecs module (`ecs`): native components under `sage.ui` (`Rect`, `Box`, `Flow`, `Paint`, `Label`, `ZIndex`, `Absolute`, `Widget`, `Toggle`, `Slider`, the events `Clicked`, `Toggled`, `Changed`, `TextChanged`, `Submitted`, `Focused`, and the singletons `Pointer`, `Nav`, `TextEntry`, `Screen`, `Interaction`). `scripts/widgets.py` defines the widget prefabs and `world.ui`. Authored components are the creator's; `Rect` and the interaction flags are computed (design doc 9.2).
- `sage_core::host`: `UiFrame`, `UiDraw` and the UI font metrics. `sage_platform::sprite::compose_ui` turns a frame into a sprite batch.
- `examples/pong`: `Pong::save` and `Pong::load`. `examples/ui_demo`: a settings menu in Python; `cargo run -p ui_demo -- render --out FILE.png`.

## Gaps and things I could not check
- **Pointer input is not recorded.** `sage-input v1` holds only button bits, so a replay can drive the UI by keyboard or gamepad (navigation bits) but not by pointer. Recording the pointer needs a format change, which is Morgan's to approve. The `Pointer`, `Nav`, `TextEntry` and `Screen` singletons are set by the host and are part of the hashed world state.
- **No windowed UI.** The platform layer has no mouse or touch input (M1 listed this as open), so UI is verified through rendering to an image, not in a live window. Pong has no UI.
- **Hierarchy:** see decision 2. No reordering, no `ZIndex`-independent tab order beyond the `tab` field, no style inheritance (design doc 9.2's style-resolve system is not built), no scrolling, tooltips, drag-reorder or animation.
- **Text:** the built-in 5x7 face only, no shaping, wrapping, selection, caret movement, clipboard or IME. A text input appends at the end and backspaces.
- **Scenes:** full values only (decision 3); loading replaces the world's entities rather than adding to them; no cooked binary form; the 2D/3D mode is stored but unused. Migrations are declarative steps, not scripts, so a conversion the five operations cannot express needs a new operation.
- **State the binding does not know** (opaque Python components, native tables like Pong's contacts) is not saved by `save_game`; the host passes it as `extra`. Opaque components are still skipped by hashing and snapshots ([Open], design doc 3.3).
- `load_scene` and `load_game` must be called between ticks, not from a system.
- `Ecs::field` still returns an `f64` (lossy above 2^53); `field_u64` is exact for ids.
- GPU rendering of UI frames goes through the same `SpriteBatch` as M1's sprites and is covered by the CPU reference; it was not run on a GPU here.
