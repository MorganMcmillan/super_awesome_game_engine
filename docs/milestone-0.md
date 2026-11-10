# Milestone 0: status

Date: 2026-11-09. Scope: design doc 13.1.

## Acceptance

| | Assertion | Status | Checked by |
|---|---|---|---|
| A1 | Replaying one recorded input file twice gives the same world-state hash at every tick; the harness catches a deliberate nondeterminism canary | **Pass** (in-process and across processes) | `examples/pong/tests/acceptance.rs` (`a1_*`), `examples/pong/tests/cli.rs` |
| A2 | A scripted headless run reaches `GameOver` with the expected winner and score; a windowed run opens and draws the court | **Pass** on Windows and Linux. Headless: left bot vs idle right ends 5:0, winner left, for seeds 1, 2, 3, 99 and 2026. Windowed: `pong smoke` (Linux under xvfb) | `a2_*` tests; `a2_window_opens_and_draws_court` (ignored by default, needs a display) |
| A3 | To be chosen by Morgan (exercise 16) | **Pass.** Morgan chose both candidates (2026-11-09). C1, contact invariants: over five full games, begin/stay/end equal the set algebra of 8.2, each begin produces exactly one `ContactBegin` event in canonical order and none while `play` is disabled, and each ball-paddle contact causes exactly one bounce (no ball-paddle stay ever occurs). C2, snapshot restore: restoring a tick-N snapshot, into a fresh world or one that has moved on, reproduces tick N's hash and every later hash; a snapshot missing the previous contact set is detected | `examples/pong/tests/a3_candidates.rs` |
| A4 | The workspace builds and tests pass in CI on Windows and Linux | **Pass locally on both platforms; hosted CI not yet run.** Every step of `.github/workflows/ci.yml` was run by hand: on Windows 11, and on Linux (Ubuntu 22.04 under WSL2, Python 3.10, deps installed as in the workflow). fmt, clippy `-D warnings`, 51 debug and 51 release tests, and the windowed smoke test all pass on both. The GitHub Actions run itself waits on a remote | `.github/workflows/ci.yml` |

## In-scope items (13.1)
- Fixed-timestep loop with a tick counter, a seeded RNG owned by the world, input recorded per tick, a world-state hash, replay from a recorded input file: `sage_core`.
- Pong in Flecs: ball, two paddles, two sides and players. `Serving`/`Playing`/`GameOver` are a `GameState` singleton, not pipeline phases. The `play` flecs module is enabled only while `Playing` (pause by module disabling, 5.5): `examples/pong/src/sim.rs`.
- Collision: AABB overlap in an ECS system (`DetectContacts`, phase `SimDetect`) that writes the contact table in canonical order and emits one `ContactBegin` Flecs event per begin transition: `sage_collision`.
- One Python rule through PyO3 for scoring (`examples/pong/scripts/rules.py`, `@sage.on("goal")`), with a fixed hash seed. The scorer is found through an `(Opponent, player)` relationship on the side entity (Appendix A, `side.opponent`).
- Presentation: SDL3 window, keyboard input and rectangle drawing behind the `Renderer` trait (Render API, per S5). Only `pong`, the app, depends on `sage_platform`; the simulation crates never do.

## Spikes gating M0
S1 pass (one gap: `Parent`) · S2 pass · S3 pass · S4 pass · S5 partial (Windows and Linux pass; GPU triangle not attempted) · S8 pass. See `docs/spikes/`. S6 and S7 were not started; they do not block M0.

## Placeholders that need Morgan
- **Bounce on begin vs stay** (8.2 [Open], exercise 8). Pong bounces on begin only, and pushes the ball out of the paddle so that a stay never happens.
- **`Parent` unavailable** at the pinned Flecs (S1). Choose between waiting for an upstream release, forking the bindings, or an owner component before the M2 UI hierarchy.
- **Codename** `sage` is a stand-in until the name is chosen (15). It lives in `sage_core::CODENAME`, `sage_script::PY_PACKAGE` and the crate prefixes.
- **A remote** so the GitHub Actions workflow can run (A4's hosted half).
- **Design docs** stay in `claude/` until Morgan agrees to move them to `docs/design.md`.
