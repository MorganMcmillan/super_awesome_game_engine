# Milestone 1: status

Date: 2026-11-10. Scope: design doc 13.3, "2D renderer on the SDL GPU API, asset pipeline basics, audio, input mapping".

Milestone 0's A4 passed on hosted CI at 054a14e; the M1 commit has not run on CI yet, and Linux is unverified for M1 (Morgan: verify later). M1 was started at Morgan's request. Decisions Morgan made for this milestone:
- Start M1 now, before M0 is accepted.
- New dependencies: "minimal", meaning only a shader step and a PNG decoder. Audio is WAV only.
- Renderer scope: sprite batching, an orthographic camera with a pixel-perfect mode, and bitmap-font text.

The design doc gives M1 no acceptance list, so the checks below are mine. Please confirm or replace them, the way exercise 16 did for M0.

## Checks

| | Claim | Status | Checked by |
|---|---|---|---|
| B1 | The GPU renderer draws the same pixels as an independent CPU rasterizer of the same sprite batch, including textures and alpha blending | **Pass on Windows (Vulkan)**; skips with a message where there is no GPU, fails instead when `SAGE_REQUIRE_GPU=1` | `crates/sage_platform/tests/gpu.rs` |
| B2 | Pixel-perfect mode uses an integer scale, whole-pixel edges (neighbors share edges) and keeps the goal-zone margin on screen | **Pass** | `sprite::tests` |
| B3 | Batching merges same-texture runs and keeps draw order; text lays out on the glyph grid | **Pass** | `sprite::tests`, `sage_assets` font tests |
| B4 | Asset ids depend only on the normalized path; the catalog is sorted; the cache decodes once and `reload` finds changed files by content hash; PNG and WAV round-trip; bad files give errors that name the path | **Pass** | `crates/sage_assets` tests |
| B5 | The mixer sums, clamps, loops, resamples and drops finished voices | **Pass** | `mixer::tests` |
| B6 | Sound cues in a full Pong game match its events (one `score` per point, one `win`, last), repeat exactly for a seed, and do not change the state hash | **Pass** | `examples/pong/tests/cues.rs` |
| B7 | Input mapping: keys, gamepad buttons and stick directions set bits; a released binding cannot cancel a held one; rebinding and the text form round-trip | **Pass** (with a fake input source) | `input::tests` |
| B8 | A real window on the GPU renderer: Pong draws court, walls, paddle and ball, opens an audio device and plays a cue | **Pass on Windows** (`pong smoke --renderer gpu`) | `pong smoke` |
| B9 | The checked-in SPIR-V and Pong assets equal what their generators produce | **Pass** | `tools/shaderc` and `pong::assets` tests |

M0's checks (A1 to A3) and the S8 debug/release parity still pass: `cargo test --workspace` and `--release`, clippy with `-D warnings` and `cargo fmt --check` are clean on Windows.

## What was built
- `crates/sage_assets` (no window or device needed): `AssetId` (FNV-1a of the normalized path), `Image` (PNG), `Sound` (WAV decode and encode, mono `f32`), `BitmapFont` (a built-in 5x7 face generated in code, upper case only; any atlas image with a grid works), and `AssetStore` (sorted catalog, content-hash cache, `reload`).
- `crates/sage_platform`: `sprite` (camera `View`, `SpriteBatch`, `compose`, a CPU reference `rasterize`), `gpu` (`GpuRenderer`: SPIR-V pipeline with alpha blending, batched upload, offscreen render and readback), `mixer` and `audio` (software mixer on one SDL stream), `input` (`InputMap`, `Binding`, rebinding, `sage-bindings v1` text). `run_windowed` picks the GPU renderer, falls back to `SdlRenderer`, opens audio, reads gamepads (with hotplug), and calls `Presentable::take_cues` after every step.
- `tools/shaderc`: assembles the two sprite shaders (see design doc 3.1 for why they are not compiled from source). `cargo run -p shaderc` rewrites `crates/sage_platform/shaders/`.
- Pong: the ball is a sprite, four sound cues, a gamepad on the left paddle, `pong gen-assets`, `--renderer gpu|sdl|auto`, `--mute`.

## Gaps and things I could not check
- **Vulkan only.** The shaders are SPIR-V. Machines without Vulkan use the Render API fallback; D3D12 and Metal are unsupported. GitHub's hosted runners have no GPU, so CI runs the fallback, and the GPU tests skip there.
- **Linux:** M1 has not been built or run on Linux (the M0 pass did it under WSL2). The new code has no Windows-only parts, but that is unverified.
- **Sound by ear:** I confirmed that the device opens, the cue is found and the mixer output is right, but I cannot hear it. Please play a game.
- **Gamepad:** only tested with a fake source. No controller was attached.
- **Not built:** atlases and sprite sheets, animation, a sprite component in the simulation (Pong's sprite is chosen by `DrawKind`), layers and sort keys, hot reload wired to a file watcher (`reload` exists), OGG or MP3, audio buses and spatialization, mouse and touch, shaped or non-ASCII text, a rebinding screen.
- The mixer assumes a 48 kHz stereo device and about 60 ms of queued audio; latency was not measured.
- The pixel counts in `pong smoke` come from an offscreen render of the same frame, because swapchain textures cannot be read back.
- `docs/dependencies.md` records the `image` crate and its transitive licenses (all permissive).
