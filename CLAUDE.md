# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is
A game engine for rapid prototyping with a low skill floor and a high ceiling. Games are small databases: entities are rows, components are columns, systems are queries. Rust core, Flecs ECS, Python scripting through PyO3, SDL3 for windowing, input, audio and graphics. A Scratch-like block editor is a *view* of plain Python. The engine's name is undecided; the codename is `sage` (see "Naming").

## Source of truth
- `claude/game-engine-design-doc.md` is the design. Read "How to use this document" and the tag meanings first. The design doc plans to move it to `docs/design.md`; ask Morgan before moving it. Until then the `claude/` copy is authoritative.
- `claude/design-exercises.md` is design homework for Morgan ("exercise N" in the design doc). Do not attempt it.
- `docs/milestone-0.md`: acceptance status and the placeholders waiting on Morgan. `docs/spikes/<id>.md`: spike write-ups. `docs/dependencies.md`: pins and licenses.
- The Python–Flecs binding (`sage_ecs_py`, design doc 3.1 and 5.4, revision 4) and Pong written in Python (`examples/pong`) were added at Morgan's request while Milestone 0 is open.
- Milestone 1 (design doc 13.3: GPU sprite renderer, asset pipeline basics, audio, input mapping) was added at Morgan's request while Milestone 0 is open. Status, checks and gaps: `docs/milestone-1.md`. M1's renderer scope is sprites, a pixel-perfect camera and bitmap text; materials, lighting and a shader toolchain are not started.
- Milestone 2 (design doc 13.3: UI module, scene format with migrations, save games) was added at Morgan's request while Milestone 0 is open. Status, checks, the decisions Morgan confirmed (three new text formats, `ChildOf` for UI) and gaps: `docs/milestone-2.md`.
- Milestone 0 (design doc 13.1) is implemented: A1–A3 pass (A3 = both candidates, chosen by Morgan); A4 passes (hosted CI on Windows and Linux succeeded at 054a14e). Section 13.2 lists the non-goals that still apply until Milestone 0 is accepted.

## Commands
- Build: `cargo build` · Test: `cargo test --workspace` · One test: `cargo test -p <crate> <name_substring>` (for example `cargo test -p pong canary`)
- Release tests (S8 requires debug/release parity): `cargo test --workspace --release`
- Needs a display (A2 windowed half): `cargo run -p pong -- smoke`
- Lint and format (CI uses both): `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`
- Run Pong: `cargo run -p pong -- play` (W/S and Up/Down, Esc quits). Also `headless [--seed N] [--record FILE]`, `replay FILE [--hashes OUT]`, `smoke [--frames N]`.
- Regenerate the sprite shaders after editing `tools/shaderc`: `cargo run -p shaderc` (a test fails when they are stale). Regenerate Pong's assets: `cargo run -p pong -- gen-assets`.
- GPU tests need a Vulkan device and skip without one; `SAGE_REQUIRE_GPU=1 cargo test -p sage_platform --test gpu` makes a missing GPU an error. Pong: `play` and `smoke` take `--renderer gpu|sdl|auto` and `--mute`.
- UI demo: `cargo run -p ui_demo -- render --out ui.png` (CPU render, no window). Regenerate the `sage.ui` editor stub with `UPDATE_STUBS=1 cargo test -p sage_ui --test stubs`.
- S4 benchmark: `cargo run --release -p spike_pyo3_cost`
- Python system cost: `cargo test --release -p sage_ecs_py --test cost -- --ignored --nocapture`
- The toolchain is pinned to 1.99.1 in `rust-toolchain.toml`. The first build compiles Flecs C and SDL3 from source (about 6 minutes; needs CMake and a C compiler). Python 3.12 must be installed and its DLL/shared library findable at run time.

## Architecture
- `crates/sage_core`: the deterministic core, with no external dependencies. `Simulation` trait (`step(TickInput)`, `tick`, `state_hash`), `InputRecording` (text format `sage-input v1`), `Rng` (xoshiro256**), `StateHasher` (FNV-1a), the replay harness (`replay_twice`, `first_divergence`), `StableId`, `CODENAME`, `TICK_DT`. `host.rs` is the host interface: the simulation hands presentation a `Frame` snapshot, and only `TickInput` flows back.
- `crates/sage_collision`: `detect()` (sweep and prune, then the parry cuboid narrow phase; canonical `a < b` output sorted by pair) and `ContactTable` (current and previous sets; begin/stay/end by sorted merge; the previous set is hashed for rewind).
- `crates/sage_script`: embedded CPython. `init()` starts it once per process through `PyConfig` (isolated, hash seed 0). `Script::load` runs source in fresh globals and discovers `@sage.on(event)` handlers in name order. The `sage` package's `__init__.py` is embedded from `python/sage/` (`PACKAGE_SOURCE`).
- `crates/sage_ecs_py`: the Python–Flecs binding (design doc 3.1, 5.4). `Ecs::new(&world, seed, module_name)`, then `ecs.load(name, source)` runs a script with a `world` global. Rust half (`sage._ecs`): `registry.rs` (per-world state, view validity), `value.rs` (`Value` base class and `Field` descriptors), `api.rs` (`NativeWorld`, `Entity`), `callbacks.rs` (systems and observers built on the C API with query `expr`), `state.rs` (generic hash, snapshot, restore). Python half (`sage.ecs`): `python/sage/ecs.py` (`@component`, decorators, signature analysis, `World`/`FlecsModule` wrappers). Host-owned native flecs modules are added with `Ecs::add_native_module` and instantiated from Python with `import_native`.
- `python/sage/`: the whole `sage` Python package as real files, embedded by `include_str!` (`__init__.py` by `sage_script`, `ecs.py` by `sage_ecs_py`). `ecs.pyi` and `_ecs.pyi` are type stubs for editors only; keep them in step with `ecs.py` and the `#[pymethods]` in `api.rs`/`value.rs`. `_native.pyi` and `collision.pyi` are *generated* from Pong's native components; regenerate with `UPDATE_STUBS=1 cargo test -p pong --test stubs` after changing one (the test fails when they are stale). `pyrightconfig.json` puts `python/` on the editor's path; `examples/pong/scripts/__builtins__.pyi` declares the `world` global.
- `examples/pong`: Pong with every rule in `scripts/pong.py`. The host provides only the reflected native vocabulary (`sage.collision.Position`, `Collider`, `ContactBegin`, `sage.Input`, `sage.Clock`) and the native `collision` flecs module. `tests/game.rs` applies A1, A2 and A3 C2 to it.
- `crates/sage_platform`: presentation. The `Renderer` and `Presentable` traits (`Presentable::take_cues` returns the sound cues of the last step), `gpu::GpuRenderer` (SDL GPU API, Vulkan/SPIR-V only), `sprite` (camera `View`, `SpriteBatch`, `compose`, and a CPU reference `rasterize` the GPU tests compare against), `SdlRenderer` (SDL Render API, the fallback), `mixer` and `audio` (software mixer on one SDL stream), `input` (`InputMap`, `Binding`, rebinding), and `run_windowed` (a wall-clock accumulator that steps a fixed number of ticks and records input per tick). The camera is applied on the CPU, so the sprite shaders have no uniforms.
- `crates/sage_scene`: no dependencies. `Scene`, `Save`, `Migrations`, `StringTable` and the text formats `sage-scene v1`, `sage-save v1`, `sage-migrations v1` (design doc 10.4). Pure data; `sage_ecs_py` converts to and from a live world (`save_scene`, `load_scene`, `save_game`, `load_game`).
- `crates/sage_ui`: the UI module (design doc 9). `layout`, `interact` and `draw` are pure; `ecs` is the `ui` flecs module with native components under `sage.ui`; `scripts/widgets.py` defines the widget prefabs and `world.ui`. A UI node is an entity with a StableId and a `Rect`; the parent is the `ChildOf` target. Host input is the `Pointer`, `Nav`, `TextEntry` and `Screen` singletons. Presentation gets a `sage_core::host::UiFrame`, which `sage_platform::sprite::compose_ui` turns into a sprite batch.
- `crates/sage_assets`: no window or device needed. `AssetId` (hash of the normalized path), `Image` (PNG), `Sound` (WAV), `BitmapFont` (built-in 5x7 face), `AssetStore` (sorted catalog, content-hash cache). Presentation-side: simulation crates do not depend on it.
- `tools/shaderc`: assembles the two sprite shaders as SPIR-V into `crates/sage_platform/shaders/`. SDL's Vulkan backend wants combined image samplers, which the Rust compilers we tried do not emit (design doc 3.1).
- `spikes/flecs_features`: S1/S2 regression tests for the pinned Flecs. `spikes/pyo3_cost`: the S4 benchmark.
- Dependency direction: simulation crates (`sage_core`, `sage_collision`, `sage_script`, `sage_ecs_py`, `sage_scene`, `sage_ui`) never depend on `sage_platform` or `sage_assets`. Only the app (`pong`) wires them together.

## Flecs binding gotchas (flecs_ecs 0.2.2, C 4.1.2)
- Entity paths use `::` (`flecs::core::ChildOf`). A module's path includes the Rust module path, so look things up relative to the module entity.
- Tags cannot be query fields with `.each`; use `.with(Tag::id())`.
- Systems that emit events or toggle modules are `.immediate(true)`, so their effects apply in order instead of at a sync point.
- `Parent` (non-fragmenting `ChildOf`) does not exist at this version (S1).
- Disabling a module entity disables its systems and observers (S2). Pause and feature-off rely on this.
- With an `Inheritable` base, `ecs_field_size` returns the term's size, not the matched (derived) column's. Take the stride from `ecs_get_type_info(ecs_field_id(..))`.
- `ecs_emit` with an empty id list reaches only `_` (Any) observers. Emit with the entity's ids; Any observers then fire once per id, so de-duplicate on `it.event_cur`.
- To include prefabs and disabled entities in a query, add optional `Prefab` and `Disabled` terms; the `EcsQueryMatchPrefab`/`EcsQueryMatchDisabled` desc flags had no effect at this pin. `IsA` is transitive, so `(IsA, Base)` matches instances of derived prefabs.

## Python binding gotchas (`sage_ecs_py`)
- Python objects hold a registry *id*, never an `Rc`: script classes and globals are cyclic garbage that Python may free on any thread. Lookups only succeed on the world's own thread.
- A view (system parameter, `entity.get(C)` inside a callback) is valid only while its callback scope is open and no structural change has happened since. Every operation that may move component data in immediate mode must call `Registry::structural_change`.
- Python systems require `sage.StableId` and run in StableId order. Entities created natively need `Ecs::assign_sid` to be seen. `sage.StableId` is `(OnInstantiate, DontInherit)`: without that, instances would copy their prefab's StableId.
- The "currently loading world" stack is keyed by OS thread id: `threading.local` resets between `Python::attach` calls from Rust threads.
- Native components under `sage.` are importable by path (`Ecs::register_native` calls `sage.ecs._expose_native`). Modules are process-wide but classes are per world, so the module `__getattr__` resolves against the currently loading world: the import only works while a script loads. Its failures raise `ImportError`, because `from m import x` hides an `AttributeError`'s message.
- New simulation state that the binding does not know (native components without `#[flecs(meta)]` + `register_native`, or tables like `ContactTable`) must be hashed and snapshotted by the host.

## Rules of the road

### Decisions
- Build against **[Decided]** items. Verify **[Leaning]** items. Never silently resolve an **[Open]** item: propose an answer, change the tag in the design doc, and add a dated note in the same change.
- Technical claims in the design doc are hypotheses until verified against primary documentation or source at the pinned versions.
- A spike is done when it has a write-up in `docs/spikes/<id>.md` meeting its pass criteria and the design doc tags are updated.
- If a spike contradicts the design doc, report the contradiction and the evidence before building around it.

### Determinism (non-negotiable inside the simulation)
- No wall-clock time or OS randomness. The world owns a seeded RNG.
- Fixed timestep. Input is data, recorded per tick. A tick (simulation step) is not a frame (render).
- Never iterate a `HashMap`/`HashSet` (or a Python set of strings) in simulation code. Use ordered containers or sort by `StableId`. The Pong canary (`nondeterminism_canary`) shows what the harness catches.
- Anything feeding collision, physics or hashing arrives sorted by `StableId`. Flecs entity IDs are not stable across runs, and Flecs iteration order changes with archetypes.
- New simulation state must be added to `state_hash`, or replays can diverge undetected.
- The Python hash seed is fixed by `sage_script::init` (the equivalent of `PYTHONHASHSEED=0`). Never initialize Python any other way.

### Ask Morgan first
- Adding a dependency (check its license; record it in `docs/dependencies.md`).
- Changing a file format (for example `sage-input v1`) or a public component schema.
- Anything that touches a **[Decided]** item.
- Anything outside Milestone 0's scope while Milestone 0 is open (13.2).

### Never
- Never run untrusted Python (kits, scripts from the network) outside a sandbox. `sage_script` has none.
- Never hardcode the placeholder project name; use the codename constants.
- Never commit large binary assets or unlicensed art.

## Testing expectations
- The determinism tests (in-process and cross-process replays, per-tick hashes) must always pass, and the canary must always be caught (S3).
- Add a regression test for every behavior the design doc says to "verify" once you have verified it.
- Collision output is checked against the brute-force reference and a golden hash under debug and release (S8). Update `GOLDEN` only on intentional changes.
- If game rules change on purpose, re-check the replay tests in `examples/pong/tests/game.rs`.

## Naming and terms
- See the glossary (design doc section 14). Key points: "pipeline phase" (Flecs `Phase`) is not a "game state" (`GameState`); do not use "stage" (Flecs uses it for command queues); say "flecs module" or "crate", never a bare "module".
- The codename `sage` is a stand-in until the name is chosen (15). It lives in `sage_core::CODENAME`, `sage_script::PY_PACKAGE` and the `sage_` crate prefix.

## Working style
- Small, reviewable changes. Update the design doc in the same change when behavior or a tag changes.
