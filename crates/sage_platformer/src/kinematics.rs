//! The native `kinematics` flecs module: one system that moves every `Body`
//! by its `Velocity`, collides it with every `Solid`, and fills in what the
//! rules need to know (`Body`). Solids with a `Velocity` are movers: they
//! carry the bodies standing on them and push the ones in their way.
//!
//! Rules (Python) decide the velocities; this decides where they lead. It
//! reads and writes entities in StableId order, so Flecs's iteration order
//! (which changes with archetypes) never matters.

use std::ffi::CString;

use flecs_ecs::prelude::*;
use flecs_ecs::sys;
use sage_collision::ecs::{Collider, Position};
use sage_core::TICK_DT;
use sage_ecs_py::StableIdC;

use crate::sweep::{BodyIn, SolidBox, push_out, step_body};
use crate::vocab::{Body, Solid, Velocity};

struct SolidRow {
    entity: u64,
    sid: u64,
    vel: Option<[f32; 2]>,
}

struct BodyRow {
    entity: u64,
    sid: u64,
    pos: [f32; 2],
    half: [f32; 2],
    vel: [f32; 2],
    body: Body,
}

/// One tick of movement for every body and mover in the world.
fn step(world: &WorldRef<'_>) {
    let mut solids: Vec<(SolidRow, SolidBox)> = vec![];
    world
        .new_query::<(&StableIdC, &Position, &Collider, &Solid)>()
        .each_entity(|e, (sid, pos, col, solid)| {
            let vel = e.try_get::<&Velocity>(|v| [v.x, v.y]);
            solids.push((
                SolidRow {
                    entity: *e.id(),
                    sid: sid.value,
                    vel,
                },
                SolidBox {
                    id: sid.value,
                    center: [pos.x, pos.y],
                    half: [col.hw, col.hh],
                    one_way: solid.one_way != 0,
                },
            ));
        });
    solids.sort_by_key(|s| s.0.sid);
    let mut bodies: Vec<BodyRow> = vec![];
    world
        .new_query::<(&StableIdC, &Position, &Collider, &Velocity, &Body)>()
        .each_entity(|e, (sid, pos, col, vel, body)| {
            bodies.push(BodyRow {
                entity: *e.id(),
                sid: sid.value,
                pos: [pos.x, pos.y],
                half: [col.hw, col.hh],
                vel: [vel.x, vel.y],
                body: *body,
            });
        });
    bodies.sort_by_key(|b| b.sid);
    let mut boxes: Vec<SolidBox> = solids.iter().map(|s| s.1).collect();
    let mut squeezed = vec![false; bodies.len()];

    // Movers go first. Riders (grounded on the mover last tick) are carried;
    // everyone else the mover now overlaps is pushed along its motion.
    for i in 0..solids.len() {
        let Some(v) = solids[i].0.vel else { continue };
        let d = [v[0] * TICK_DT, v[1] * TICK_DT];
        if d == [0.0, 0.0] {
            continue;
        }
        let carried: Vec<bool> = bodies
            .iter()
            .map(|b| b.body.grounded != 0 && b.body.floor == solids[i].0.sid)
            .collect();
        boxes[i].center = [boxes[i].center[0] + d[0], boxes[i].center[1] + d[1]];
        for (r, b) in bodies.iter_mut().enumerate() {
            if carried[r] {
                let out = step_body(
                    &boxes,
                    BodyIn {
                        center: b.pos,
                        half: b.half,
                        delta: d,
                        drop_through: false,
                    },
                );
                b.pos = out.center;
            } else {
                let (p, squeeze) = push_out(&boxes, &boxes[i], b.pos, b.half, d);
                b.pos = p;
                squeezed[r] |= squeeze;
            }
        }
    }
    for (row, boxed) in solids.iter().zip(&boxes) {
        if row.0.vel.is_some() {
            world.entity_from_id(row.0.entity).set(Position {
                x: boxed.center[0],
                y: boxed.center[1],
            });
        }
    }

    // Then every body moves itself.
    for (r, b) in bodies.iter().enumerate() {
        let out = step_body(
            &boxes,
            BodyIn {
                center: b.pos,
                half: b.half,
                delta: [b.vel[0] * TICK_DT, b.vel[1] * TICK_DT],
                drop_through: b.body.drop_through != 0,
            },
        );
        let vel = Velocity {
            x: if out.hit_x { 0.0 } else { b.vel[0] },
            y: if out.hit_up || out.hit_down {
                0.0
            } else {
                b.vel[1]
            },
        };
        let body = Body {
            floor: out.floor,
            grounded: out.grounded as u32,
            ceiling: out.ceiling as u32,
            wall: out.wall,
            headroom: out.headroom,
            drop_left: out.drop_left as u32,
            drop_right: out.drop_right as u32,
            drop_through: 0,
            squeezed: squeezed[r] as u32,
        };
        let e = world.entity_from_id(b.entity);
        e.set(Position {
            x: out.center[0],
            y: out.center[1],
        });
        e.set(vel);
        e.set(body);
    }
}

/// The native `kinematics` flecs module: `Move` in `phase`, under `parent`.
pub fn install_kinematics(
    w: *mut sys::ecs_world_t,
    phase: u64,
    parent: u64,
) -> Result<u64, String> {
    let world = unsafe { WorldRef::from_ptr(w) };
    let named = |parent: u64, name: &str| {
        let name = CString::new(name).unwrap();
        let desc = sys::ecs_entity_desc_t {
            name: name.as_ptr(),
            parent,
            sep: c"".as_ptr(),
            ..Default::default()
        };
        unsafe { sys::ecs_entity_init(w, &desc) }
    };
    let module = named(parent, "kinematics");
    unsafe { sys::ecs_add_id(w, module, sys::EcsModule) };
    let prev = unsafe { sys::ecs_set_scope(w, module) };
    // Immediate: it reads and writes many entities in order, and the rules
    // that run after it must see the result.
    world
        .system_named::<(&StableIdC, &Position, &Collider, &Solid)>("Move")
        .kind(phase)
        .immediate(true)
        .run(move |mut it| {
            while it.next() {}
            step(&it.world());
        });
    unsafe { sys::ecs_set_scope(w, prev) };
    Ok(module)
}
