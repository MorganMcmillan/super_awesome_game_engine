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

**Transitive audit (2026-11-09, `cargo metadata`):** 89 crates in normal and build dependencies. All are permissive: MIT, Apache-2.0, MIT OR Apache-2.0, Zlib, BSD-3-Clause (`bindgen`, build only), ISC (`libloading`), BSL-1.0 OR Apache-2.0 (`ryu`), Unlicense OR MIT, Unicode-3.0. None is copyleft.

**Build requirements:** CMake and a C compiler (Flecs C and SDL3 are built from source); Python 3.12 with its shared library; on Linux, the X11/Wayland/audio development packages listed in `.github/workflows/ci.yml`.
