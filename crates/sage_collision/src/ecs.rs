//! The collision vocabulary as Flecs components, and the native `collision`
//! flecs module that announces contacts.
//!
//! Pong and the platformer kit share this: `sage.collision.Position`,
//! `Collider` and the `ContactBegin` event, the host's `sage.Input` and
//! `sage.Clock` singletons, and one native module, `collision`, that scripts
//! instantiate inside their own flecs module (`play.import_native("collision",
//! phase=...)`), so disabling that module also disables detection.
//!
//! Usage, in this order (the binding needs the component ids before it
//! exists, and the module closure after):
//!
//! ```ignore
//! let vocab = Vocabulary::define(&world);
//! let ecs = Ecs::new(&world, seed, "game")?;
//! vocab.install(&world, &ecs)?;
//! ```

use std::ffi::CString;

use flecs_ecs::prelude::*;
use flecs_ecs::sys;
use sage_core::StableId;
use sage_ecs_py::{Ecs, StableIdC};

use crate::{BoxCollider, ContactTable, detect};

#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
#[flecs(meta)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// Axis-aligned box collider, by half extents.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
#[flecs(meta)]
pub struct Collider {
    pub hw: f32,
    pub hh: f32,
}

/// One per begin transition of the contact table, in canonical order.
/// `a < b` are StableIds; the normal points from `a` to `b`.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
#[flecs(meta)]
pub struct ContactBegin {
    pub a: u64,
    pub b: u64,
    pub nx: f32,
    pub ny: f32,
    pub depth: f32,
}

/// The host's input for this tick: held button bits.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
#[flecs(meta)]
pub struct Input {
    pub buttons: u32,
}

/// The tick counter, advanced by the host before each step.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
#[flecs(meta)]
pub struct Clock {
    pub tick: u64,
}

/// The contact table: native state, hashed and snapshotted by the host.
#[derive(Component, Debug, Clone, Default)]
pub struct Contacts(pub ContactTable);

/// The component ids, defined before the binding is created.
pub struct Vocabulary {
    ids: [u64; 5],
}

impl Vocabulary {
    pub fn define(world: &World) -> Vocabulary {
        Vocabulary {
            ids: [
                *world
                    .component_named::<Position>("sage::collision::Position")
                    .id(),
                *world
                    .component_named::<Collider>("sage::collision::Collider")
                    .id(),
                *world
                    .component_named::<ContactBegin>("sage::collision::ContactBegin")
                    .id(),
                *world.component_named::<Input>("sage::Input").id(),
                *world.component_named::<Clock>("sage::Clock").id(),
            ],
        }
    }

    /// Registers the components with the binding, creates the singletons and
    /// makes the `collision` flecs module available to scripts.
    pub fn install(&self, world: &World, ecs: &Ecs) -> Result<(), String> {
        for id in self.ids {
            ecs.register_native(id)?;
        }
        world.set(Input::default());
        world.set(Clock::default());
        world.set(Contacts(ContactTable::new()));
        ecs.add_native_module("collision", install_collision);
        Ok(())
    }
}

/// The native `collision` flecs module: `DetectContacts` in `phase`, under
/// `parent`, announcing begin transitions as `ContactBegin` events.
fn install_collision(w: *mut sys::ecs_world_t, phase: u64, parent: u64) -> Result<u64, String> {
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
    let module = named(parent, "collision");
    unsafe { sys::ecs_add_id(w, module, sys::EcsModule) };
    let announcer = named(module, "Announcer");
    let prev = unsafe { sys::ecs_set_scope(w, module) };
    // Immediate: observers of ContactBegin (Python rules) change things
    // directly, in canonical contact order. Flecs iteration order does not
    // matter: `detect` sorts by StableId.
    world
        .system_named::<(&StableIdC, &Position, &Collider)>("DetectContacts")
        .kind(phase)
        .immediate(true)
        .run(move |mut it| {
            let mut colliders = vec![];
            while it.next() {
                let sid = it.field::<StableIdC>(0);
                let pos = it.field::<Position>(1);
                let col = it.field::<Collider>(2);
                for i in it.iter() {
                    colliders.push(BoxCollider {
                        id: StableId(sid[i].value),
                        center: [pos[i].x, pos[i].y],
                        half: [col[i].hw, col[i].hh],
                    });
                }
            }
            let world = it.world();
            let tick = world.get::<&Clock>(|c| c.tick);
            let begins = world.get::<&mut Contacts>(|t| {
                t.0.advance(detect(tick, &colliders));
                t.0.transitions().begin
            });
            let announcer = world.entity_from_id(announcer);
            for c in begins {
                announcer.emit(&ContactBegin {
                    a: c.a.0,
                    b: c.b.0,
                    nx: c.normal[0],
                    ny: c.normal[1],
                    depth: c.depth,
                });
            }
        });
    unsafe { sys::ecs_set_scope(w, prev) };
    Ok(module)
}
