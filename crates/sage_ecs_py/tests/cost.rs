//! Per-entity cost of a Python system, to compare with S4's column A (one
//! Python call per entity over Rust Vecs: 0.34 ms at 1k, 3.97 ms at 10k).
//! Reported, not gated. Run with:
//! `cargo test --release -p sage_ecs_py --test cost -- --ignored --nocapture`

use std::time::Instant;

use flecs_ecs::prelude::*;
use sage_ecs_py::Ecs;

const SRC: &str = r#"
import sage
from sage import component

@component
class Pos:
    x: sage.f32 = 0.0
    y: sage.f32 = 0.0

@component
class Vel:
    x: sage.f32 = 1.0
    y: sage.f32 = 1.0

for i in range(N):
    world.entity().set(Pos(i, i)).set(Vel())

@world.system
def integrate(pos: Pos, vel: Vel):
    pos.x += vel.x * sage.TICK_DT
    pos.y += vel.y * sage.TICK_DT
"#;

#[test]
#[ignore]
fn python_system_cost_per_tick() {
    for n in [1_000, 10_000] {
        let world = World::new();
        let ecs = Ecs::new(&world, 1, "bench").unwrap();
        let _script = ecs
            .load("bench", &SRC.replace("N)", &format!("{n})")))
            .unwrap();
        world.progress(); // warm up (first run sorts by StableId)
        let ticks = 50;
        let start = Instant::now();
        for _ in 0..ticks {
            world.progress();
        }
        let per_tick = start.elapsed().as_secs_f64() * 1000.0 / ticks as f64;
        assert!(ecs.fault().is_none());
        println!(
            "N={n}: {per_tick:.2} ms per tick, {:.2} us per entity",
            per_tick * 1000.0 / n as f64
        );
        drop(ecs);
    }
}
