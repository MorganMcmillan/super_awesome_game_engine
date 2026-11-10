# CLAUDE.md (draft: copy to the repo root once the repo exists)

Items marked TODO are unknown until the Phase 0 spikes report. Do not guess them.

## What this project is
A game engine for rapid prototyping with a low skill floor and a high ceiling. Games are small databases: entities are rows, components are columns, systems are queries. Rust core, Flecs ECS, Python scripting through PyO3, SDL3 for windowing/input/audio/graphics. A Scratch-like block editor is a *view* of plain Python. The engine's name is undecided; see "Naming".

## Source of truth
- `docs/design.md` is the design (mirror: `claude/game-engine-design-doc.md` in the Claude Project). Read "How to use this document" and the tags first.
- `docs/design-exercises.md` is design homework for Morgan. Do not attempt it.
- Your work starts at section 13 of the design doc: Milestone 0, then the Phase 0 spikes.

## Stack and pins
- Rust (version: TODO), Flecs through the Rust bindings (crate and version: TODO, spike S1), PyO3 (TODO), CPython (TODO), SDL3 bindings (TODO, spike S5), parry (TODO, spike S8).
- Pin everything. Flecs's newer features (`Parent`, `OrderedChildren`, `DontFragment`) are young; do not bump the Flecs version without re-running the tests for them.

## Repo layout (proposal, adjust after S1)
- Cargo workspace. Crates: `core` (world, tick loop, input recording, hashing), `collision` (contact table), `script` (PyO3), `platform` (SDL3, `Renderer` trait), and a thin app crate.
- `docs/` (design, spike write-ups in `docs/spikes/`), `tests/`, `examples/pong/`.
- The simulation crates must never depend on the presentation crate.

## Commands
- TODO: fill in build, test and run commands once the workspace exists. Prefer plain `cargo` commands. CI runs on Windows and Linux.

## Rules of the road

### Decisions
- Build against **[Decided]** items. Verify **[Leaning]** items. Never silently resolve an **[Open]** item: propose an answer, change the tag in `docs/design.md`, and add a dated note in the same change.
- Technical claims in the design doc are hypotheses until verified against primary documentation or source at the pinned versions.
- A spike is done when it has a write-up in `docs/spikes/<id>.md` meeting its pass criteria and the design doc tags are updated.

### Determinism (non-negotiable inside the simulation)
- No wall-clock time or OS randomness. The world owns a seeded RNG.
- Fixed timestep. Input is data, recorded per tick.
- Never iterate a `HashMap`/`HashSet` (or a Python set of strings) in simulation code. Use ordered containers or sort by stable ID.
- Anything that feeds collision or physics arrives in canonical order (sorted by stable ID), because Flecs iteration order changes as entities change archetype.
- Set `PYTHONHASHSEED` for embedded Python.
- Presentation (render, audio, window) only reads simulation state and never writes it.

### Ask Morgan first
- Adding a dependency (check its license).
- Changing a file format or a public component schema.
- Anything that touches a **[Decided]** item.
- Anything outside Milestone 0's scope while Milestone 0 is open (see the non-goals in 13.2).

### Never
- Never run untrusted Python (kits, scripts from the network) outside a sandbox.
- Never hardcode the placeholder project name; use the single codename constant.
- Never commit large binary assets or unlicensed art.

## Testing expectations
- A determinism test (replay twice, compare per-tick world hashes) must always pass, and its canary must always fail when it should (spike S3).
- Add a regression test for every behavior the design doc says to "verify" once you have verified it (for example, disabling a flecs module disables its observers: S2).
- Collision output is checked against a brute-force reference (spike S8).

## Naming and terms
- See the glossary (design doc section 14). Key points: "pipeline phase" (Flecs `Phase`) is not a "game state" (`GameState`); do not use "stage" (Flecs uses it for command queues); say "flecs module" or "crate", never a bare "module".
- The project's name is undecided. Use one codename constant for crate names, the Python package name and decorators (the design doc's `@game_engine.position` is a placeholder).

## Working style
- Small, reviewable changes. Update the design doc in the same change when behavior or a tag changes.
- If a spike contradicts the design doc, report the contradiction and the evidence before building around it.
