//! Spike S2: disabling a flecs module disables its systems AND its observers
//! (maintainer-confirmed, design doc 5.5). Regression test; re-run before
//! bumping Flecs. Write-up: docs/spikes/S2.md.
//!
//! The decision that pause and feature-off are done by disabling modules
//! (5.5, [Decided]) depends on this.

use std::cell::Cell;
use std::rc::Rc;

use flecs_ecs::prelude::*;

#[derive(Component)]
struct Bump;

#[derive(Component, Default)]
struct Counters;

thread_local! {
    static SYSTEM_RUNS: Rc<Cell<u32>> = Rc::new(Cell::new(0));
    static OBSERVER_RUNS: Rc<Cell<u32>> = Rc::new(Cell::new(0));
}

#[derive(Component)]
struct Feature;

impl Module for Feature {
    fn module(world: &World) {
        world.module::<Feature>("feature");
        let phase = world
            .entity_named("FeaturePhase")
            .add(flecs::pipeline::Phase)
            .depends_on(flecs::pipeline::OnUpdate);
        let s = SYSTEM_RUNS.with(|c| c.clone());
        world
            .system_named::<()>("FeatureSystem")
            .kind(phase)
            .run(move |mut it| {
                while it.next() {}
                s.set(s.get() + 1)
            });
        let o = OBSERVER_RUNS.with(|c| c.clone());
        world
            .observer_named::<Bump, ()>("FeatureObserver")
            .with(flecs::Any)
            .run(move |mut it| {
                while it.next() {}
                o.set(o.get() + 1)
            });
    }
}

fn counts() -> (u32, u32) {
    (
        SYSTEM_RUNS.with(|c| c.get()),
        OBSERVER_RUNS.with(|c| c.get()),
    )
}

#[test]
fn disabling_module_disables_systems_and_observers() {
    let world = World::new();
    let module = world.import::<Feature>();
    let target = world.entity();

    world.progress();
    target.emit(&Bump);
    assert_eq!(counts(), (1, 1), "baseline: both fire while enabled");

    module.disable_self();
    world.progress();
    target.emit(&Bump);
    assert_eq!(
        counts(),
        (1, 1),
        "neither may fire while the module is disabled"
    );

    module.enable_self();
    world.progress();
    target.emit(&Bump);
    assert_eq!(counts(), (2, 2), "both fire again after re-enabling");
}
