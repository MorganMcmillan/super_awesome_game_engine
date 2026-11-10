# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is
A game engine for rapid prototyping with a low skill floor and a high ceiling. Games are small databases: entities are rows, components are columns, systems are queries. Rust core, Flecs ECS, Python scripting through PyO3, SDL3 for windowing, input, audio and graphics. A Scratch-like block editor is a *view* of plain Python. The engine's name is undecided; the codename is `sage` (see "Naming").

## Source of truth
- `claude/game-engine-design-doc.md` is the design. Read "How to use this document" and the tag meanings first. The design doc plans to move it to `docs/design.md`; ask Morgan before moving it. Until then the `claude/` copy is authoritative.
- `claude/design-exercises.md` is design homework for Morgan ("exercise N" in the design doc). Do not attempt it.
- `docs/milestone-0.md`: acceptance status and the placeholders waiting on Morgan. `docs/spikes/<id>.md`: spike write-ups. `docs/dependencies.md`: pins and licenses.
- Milestone 0 (design doc 13.1) is implemented: A1–A3 pass (A3 = both candidates, chosen by Morgan); A4 passes locally on Windows and Linux, and the hosted CI run is pending. Section 13.2 lists the non-goals that still apply until Milestone 0 is accepted.

## Commands
- Build: `cargo build` · Test: `cargo test --workspace` · One test: `cargo test -p <crate> <name_substring>` (for example `cargo test -p pong a1_canary`)
- Release tests (S8 requires debug/release parity): `cargo test --workspace --release`
- Needs a display (A2 windowed half): `cargo test -p pong --test cli -- --ignored`
- Lint and format (CI uses both): `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`
- Run Pong: `cargo run -p pong -- play` (W/S and Up/Down, Esc quits). Also `headless [--seed N] [--record FILE]`, `replay FILE [--hashes OUT]`, `smoke [--frames N]`.
- S4 benchmark: `cargo run --release -p spike_pyo3_cost`
- The toolchain is pinned to 1.99.1 in `rust-toolchain.toml`. The first build compiles Flecs C and SDL3 from source (about 6 minutes; needs CMake and a C compiler). Python 3.12 must be installed and its DLL/shared library findable at run time.

## Architecture
- `crates/sage_core`: the deterministic core, with no external dependencies. `Simulation` trait (`step(TickInput)`, `tick`, `state_hash`), `InputRecording` (text format `sage-input v1`), `Rng` (xoshiro256**), `StateHasher` (FNV-1a), the replay harness (`replay_twice`, `first_divergence`), `StableId`, `CODENAME`, `TICK_DT`. `host.rs` is the host interface: the simulation hands presentation a `Frame` snapshot, and only `TickInput` flows back.
- `crates/sage_collision`: `detect()` (sweep and prune, then the parry cuboid narrow phase; canonical `a < b` output sorted by pair) and `ContactTable` (current and previous sets; begin/stay/end by sorted merge; the previous set is hashed for rewind).
- `crates/sage_script`: embedded CPython. `init()` starts it once per process through `PyConfig` (isolated, hash seed 0). `Script::load` runs source in fresh globals and discovers `@sage.on(event)` handlers in name order. The `sage` Python package is defined inline (`PACKAGE_SOURCE`).
- `crates/sage_platform`: presentation. The `Renderer` and `Presentable` traits, `SdlRenderer` (SDL Render API, the throwaway renderer for M0), `InputMap`, and `run_windowed` (a wall-clock accumulator that steps a fixed number of ticks and records input per tick).
- `examples/pong`: the Milestone 0 game. `src/sim.rs` holds the Flecs world: flecs modules `PongCore` (always on: clock, game-state machine) and `PongPlay` (enabled only while `Playing`: input, movement, `DetectContacts`, and the observers `BounceBall`, `BallHitsSide`, `OnGoal`). Custom phases run SimInput → SimMove → SimDetect → SimReact after OnUpdate. `scripts/rules.py` is the Python scoring rule. `tests/acceptance.rs` and `tests/cli.rs` cover A1 and A2; `tests/a3_candidates.rs` is A3 (both candidates, C1 and C2). `Pong::snapshot`/`restore` must cover everything `state_hash` covers.
- `spikes/flecs_features`: S1/S2 regression tests for the pinned Flecs. `spikes/pyo3_cost`: the S4 benchmark.
- Dependency direction: simulation crates (`sage_core`, `sage_collision`, `sage_script`) never depend on `sage_platform`. Only the app (`pong`) wires them together.

## Flecs binding gotchas (flecs_ecs 0.2.2, C 4.1.2)
- Entity paths use `::` (`flecs::core::ChildOf`). A module's path includes the Rust module path, so look things up relative to the module entity.
- Tags cannot be query fields with `.each`; use `.with(Tag::id())`.
- Systems that emit events or toggle modules are `.immediate(true)`, so their effects apply in order instead of at a sync point.
- `Parent` (non-fragmenting `ChildOf`) does not exist at this version (S1).
- Disabling a module entity disables its systems and observers (S2). Pause and feature-off rely on this.

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
- If game rules change on purpose, re-record `examples/pong/tests/data/seed7_left_bot.txt` with `pong headless --seed 7 --record ...`.

## Naming and terms
- See the glossary (design doc section 14). Key points: "pipeline phase" (Flecs `Phase`) is not a "game state" (`GameState`); do not use "stage" (Flecs uses it for command queues); say "flecs module" or "crate", never a bare "module".
- The codename `sage` is a stand-in until the name is chosen (15). It lives in `sage_core::CODENAME`, `sage_script::PY_PACKAGE` and the `sage_` crate prefix.

## Working style
- Small, reviewable changes. Update the design doc in the same change when behavior or a tag changes.
