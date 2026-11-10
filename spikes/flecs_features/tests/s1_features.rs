//! Spike S1: which Flecs features the pinned bindings reach
//! (`flecs_ecs` 0.2.2, bundling Flecs C 4.1.2). Write-up: docs/spikes/S1.md.
//!
//! These stay as regression tests: re-run them before bumping Flecs.

use std::cell::RefCell;
use std::rc::Rc;

use flecs_ecs::prelude::*;

#[derive(Debug, Component, Default, Clone, Copy, PartialEq)]
#[flecs(meta)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Component, Default, Clone, Copy)]
#[flecs(traits(DontFragment))]
pub struct Tint {
    pub v: u32,
}

#[derive(Debug, Component)]
pub struct Ping {
    pub n: u32,
}

#[test]
fn bundled_c_version_is_4_1_2() {
    assert_eq!(
        (
            flecs_ecs::sys::FLECS_VERSION_MAJOR,
            flecs_ecs::sys::FLECS_VERSION_MINOR,
            flecs_ecs::sys::FLECS_VERSION_PATCH
        ),
        (4, 1, 2)
    );
}

/// Gap: the non-fragmenting `Parent` component does not exist in Flecs C
/// 4.1.2. When this test fails after a bump, `Parent` has arrived: revisit
/// design doc 6, 9.1 and spike S7.
#[test]
fn parent_is_not_in_this_version() {
    let world = World::new();
    // Control: the same lookup finds a built-in that does exist.
    assert!(world.try_lookup("flecs::core::ChildOf").is_some());
    assert!(world.try_lookup("flecs::core::Parent").is_none());
}

#[test]
fn ordered_children_wrapped() {
    let world = World::new();
    let parent = world.entity().add(flecs::OrderedChildren);
    let a = world.entity().child_of(parent);
    let b = world.entity().child_of(parent);
    let c = world.entity().child_of(parent);
    parent.set_child_order(&[*c, *a, *b]);
    let mut seen = vec![];
    parent.each_child(|e| seen.push(*e));
    assert_eq!(seen, vec![*c, *a, *b]);
}

/// Morgan's fallback hypothesis (design doc 3.4): the trait is reachable by
/// name lookup of the built-in entity plus the generic add-by-id API.
#[test]
fn ordered_children_by_name_lookup() {
    let world = World::new();
    let trait_entity = world
        .try_lookup("flecs::core::OrderedChildren")
        .expect("built-in OrderedChildren entity exists");
    assert_eq!(*trait_entity.id(), flecs::OrderedChildren::ID);
    let parent = world.entity().add(trait_entity);
    let a = world.entity().child_of(parent);
    let b = world.entity().child_of(parent);
    parent.set_child_order(&[*b, *a]);
    let mut seen = vec![];
    parent.each_child(|e| seen.push(*e));
    assert_eq!(seen, vec![*b, *a]);
}

#[test]
fn dont_fragment_keeps_table() {
    let world = World::new();
    let e = world.entity().set(Position { x: 1.0, y: 2.0 });
    let before = e.table().map(|t| t.to_string());
    e.set(Tint { v: 7 });
    let after = e.table().map(|t| t.to_string());
    assert_eq!(before, after, "DontFragment component moved the entity");
    e.get::<&Tint>(|t| assert_eq!(t.v, 7));
    // Queries still match it.
    let mut n = 0;
    world.query::<&Tint>().build().each(|_| n += 1);
    assert_eq!(n, 1);
}

#[derive(Component)]
struct DemoModule;

impl Module for DemoModule {
    fn module(world: &World) {
        world.module::<DemoModule>("demo");
        world.component::<Position>();
        world
            .system_named::<&mut Position>("Drift")
            .each(|p| p.x += 1.0);
    }
}

#[test]
fn modules_scope_their_contents() {
    let world = World::new();
    let m = world.import::<DemoModule>();
    assert!(m.has(flecs::Module));
    let sys = m.lookup("Drift");
    assert_eq!(sys.parent().map(|p| *p.id()), Some(*m.id()));
    // A module loads once.
    let again = world.import::<DemoModule>();
    assert_eq!(*m.id(), *again.id());
}

#[test]
fn observers_with_payload() {
    let world = World::new();
    let got = Rc::new(RefCell::new(vec![]));
    let g = got.clone();
    world
        .observer::<Ping, ()>()
        .with(flecs::Any)
        .each_iter(move |it, _, _| g.borrow_mut().push(it.param().n));
    let e = world.entity();
    e.emit(&Ping { n: 3 });
    e.emit(&Ping { n: 4 });
    assert_eq!(*got.borrow(), vec![3, 4]);
}

#[test]
fn custom_phases_order_systems() {
    let world = World::new();
    let first = world
        .entity()
        .add(flecs::pipeline::Phase)
        .depends_on(flecs::pipeline::OnUpdate);
    let second = world.entity().add(flecs::pipeline::Phase).depends_on(first);
    let log = Rc::new(RefCell::new(vec![]));
    // Registered in reverse to show definition order carries no meaning.
    let l = log.clone();
    world.system::<()>().kind(second).run(move |mut it| {
        while it.next() {}
        l.borrow_mut().push("second")
    });
    let l = log.clone();
    world.system::<()>().kind(first).run(move |mut it| {
        while it.next() {}
        l.borrow_mut().push("first")
    });
    world.progress();
    assert_eq!(*log.borrow(), vec!["first", "second"]);
}

#[test]
fn disabling_a_phase_excludes_dependents_transitively() {
    let world = World::new();
    let a = world
        .entity()
        .add(flecs::pipeline::Phase)
        .depends_on(flecs::pipeline::OnUpdate);
    let b = world.entity().add(flecs::pipeline::Phase).depends_on(a);
    let ran = Rc::new(RefCell::new(0));
    let r = ran.clone();
    world.system::<()>().kind(b).run(move |mut it| {
        while it.next() {}
        *r.borrow_mut() += 1
    });
    world.progress();
    a.disable_self();
    world.progress();
    assert_eq!(*ran.borrow(), 1, "system in B ran although A was disabled");
}

#[test]
fn json_serialization_with_reflection() {
    let world = World::new();
    let e = world.entity_named("p").set(Position { x: 2.0, y: 4.0 });
    let json = e.to_json(None);
    assert!(
        json.contains("\"x\":2") && json.contains("\"y\":4"),
        "{json}"
    );
    let one = e.get::<&Position>(|p| world.to_json::<Position>(p));
    assert_eq!(one, "{\"x\":2, \"y\":4}");
}
