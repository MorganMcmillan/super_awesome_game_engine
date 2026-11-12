# Dependencies

All versions are pinned exactly in the root `Cargo.toml`, and the toolchain is pinned in `rust-toolchain.toml`. The engine's own license is still [Open] (design doc 15).

| Dependency | Version | License | Why |
|---|---|---|---|
| Rust toolchain | 1.99.1 | MIT OR Apache-2.0 | `flecs_ecs` needs edition 2024 |
| `flecs_ecs` (bundles Flecs C 4.1.2) | =0.2.2 | MIT | ECS, [Decided] (3, 3.1); spike S1 |
| `pyo3` | =0.30.1 | MIT OR Apache-2.0 | Python scripting, [Decided] (3.1); spike S4 |
| CPython (system install, linked dynamically) | 3.12 | PSF-2.0 | Scripting runtime |
| `sdl3` + `sdl3-sys` (vendors SDL 3.4.20, static) | =0.22.1 / 0.7.3 | MIT / Zlib | Window, input, drawing, [Decided] (3.1); spike S5 |
| `parry2d` | =0.31.1 | Apache-2.0 | Collision queries, [Decided] (8.1); spike S8 |
| `image` (PNG feature only, default features off) | =0.25.10 | MIT OR Apache-2.0 | Decoding and encoding PNG for the M1 asset pipeline (`sage_assets`). Approved by Morgan 2026-11-10 |

**Transitive audit (2026-11-09, `cargo metadata`):** 89 crates in normal and build dependencies. All are permissive: MIT, Apache-2.0, MIT OR Apache-2.0, Zlib, BSD-3-Clause (`bindgen`, build only), ISC (`libloading`), BSL-1.0 OR Apache-2.0 (`ryu`), Unlicense OR MIT, Unicode-3.0. None is copyleft.

**Build requirements:** CMake and a C compiler (Flecs C and SDL3 are built from source); Python 3.12 with its shared library; on Linux, the X11/Wayland/audio development packages listed in `.github/workflows/ci.yml`.

**M1 transitive audit (2026-11-10, `cargo metadata` for `sage_assets`):** `image` pulls in 18 more crates: adler2 (0BSD OR MIT OR Apache-2.0), autocfg, bitflags, cfg-if, crc32fast, fdeflate, flate2, libm (MIT), num-traits, png (all MIT OR Apache-2.0), bytemuck (Zlib OR Apache-2.0 OR MIT), byteorder-lite (Unlicense OR MIT), miniz_oxide (MIT OR Zlib OR Apache-2.0), moxcms and pxfm (BSD-3-Clause OR Apache-2.0), simd-adler32 (MIT), zlib-rs (Zlib). None is copyleft.

No shader compiler is a dependency: `tools/shaderc` has none (design doc 3.1).

**M2 (2026-11-11):** no new external dependency. `sage_scene` has none; `sage_ui` and `examples/ui_demo` use only the dependencies above and the workspace crates.

**UI parity, dependencies approved by Morgan 2026-11-11 (decisions 1 and 4 of `docs/ui-html-parity.md`), pinned exactly in the workspace `Cargo.toml`:**
- `roxmltree` 0.21.2 (MIT OR Apache-2.0): parses SVG documents in `sage_ui::svg`.
- `accesskit` 0.25.2 (MIT OR Apache-2.0): the data model only; `sage_ui::access_kit` maps the accessibility tree onto its `TreeUpdate`. No platform adapter crate is used yet.
- `fontdue` 0.9.5 (MIT OR Apache-2.0 OR Zlib): rasterizes a TrueType face into the bitmap atlas (`BitmapFont::from_ttf` in `sage_assets`). No font file is committed.
- `lewton` 0.10.2 (MIT OR Apache-2.0): Ogg Vorbis decoding in `sage_assets::compressed`.
- `puremp3` 0.1.0 (MIT OR CC0-1.0): MP3 decoding there. It decodes MPEG-1 correctly but plays the 22 kHz variants at the wrong pitch, so those are refused with an error.
Test fixtures: three tiny generated tones (about 10 KB in all) in `crates/sage_assets/tests/data/`. Approved afterwards (same day), for the last open items:
- `accesskit_windows` 0.35.2 (MIT OR Apache-2.0), a Windows-only dependency of `sage_platform` (UI Automation adapter, `sage_platform::a11y`). Morgan approved the Windows adapter only.
- `rustybuzz` 0.20.1 (MIT): shaping (kerning, ligatures) for proportional text, in `sage_assets::proportional`.
- `unicode-segmentation` 1.13.3 (MIT OR Apache-2.0): grapheme-aware caret and delete in `sage_ui::text`.
Still **not approved** and not used: `accesskit_unix`, `accesskit_macos`.
