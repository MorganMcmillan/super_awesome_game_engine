//! Thin helpers over the Flecs C API (`flecs_ecs::sys`).
//!
//! The binding talks to Flecs through the C API because everything it does is
//! dynamic: component types, queries and systems are only known at run time.

use std::ffi::{CStr, CString};

use flecs_ecs::sys;

pub type RawWorld = *mut sys::ecs_world_t;

pub const ECS_PAIR: u64 = 1u64 << 63;
const COMPONENT_MASK: u64 = !(0xFFu64 << 56);

pub fn cstring(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|_| CString::new(s.replace('\0', "")).unwrap())
}

pub fn pair(rel: u64, tgt: u64) -> u64 {
    flecs_ecs::prelude::ecs_pair(rel, tgt)
}

pub fn is_pair(id: u64) -> bool {
    id & ECS_PAIR != 0
}

pub fn pair_first(w: RawWorld, id: u64) -> u64 {
    let lo = ((id & COMPONENT_MASK) >> 32) & 0xFFFF_FFFF;
    unsafe { sys::ecs_get_alive(w, lo) }
}

pub fn pair_second(w: RawWorld, id: u64) -> u64 {
    unsafe { sys::ecs_get_alive(w, id & 0xFFFF_FFFF) }
}

/// Creates (or finds) a named child of `parent` (0 = root). The name is used
/// as is, never split on separators.
pub fn named_child(w: RawWorld, parent: u64, name: &str) -> u64 {
    let name = cstring(name);
    let desc = sys::ecs_entity_desc_t {
        name: name.as_ptr(),
        parent,
        sep: c"".as_ptr(),
        ..Default::default()
    };
    unsafe { sys::ecs_entity_init(w, &desc) }
}

pub fn lookup_child(w: RawWorld, parent: u64, name: &str) -> u64 {
    let name = cstring(name);
    unsafe { sys::ecs_lookup_child(w, parent, name.as_ptr()) }
}

/// Looks up a dotted path (`sage.physics.Position`) from the root.
pub fn lookup_path(w: RawWorld, path: &str) -> u64 {
    let mut cur = 0;
    for part in path.split('.') {
        cur = lookup_child(w, cur, part);
        if cur == 0 {
            return 0;
        }
    }
    cur
}

pub fn name(w: RawWorld, e: u64) -> Option<String> {
    let p = unsafe { sys::ecs_get_name(w, e) };
    if p.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }
}

/// The dotted path from the root, built from names (no allocation by Flecs).
pub fn path(w: RawWorld, e: u64) -> String {
    let mut parts = vec![];
    let mut cur = e;
    while cur != 0 {
        parts.push(name(w, cur).unwrap_or_else(|| format!("#{}", cur & 0xFFFF_FFFF)));
        cur = unsafe { sys::ecs_get_parent(w, cur) };
    }
    parts.reverse();
    parts.join(".")
}

pub fn has(w: RawWorld, e: u64, id: u64) -> bool {
    unsafe { sys::ecs_has_id(w, e, id) }
}

pub fn add(w: RawWorld, e: u64, id: u64) {
    unsafe { sys::ecs_add_id(w, e, id) }
}

pub fn remove(w: RawWorld, e: u64, id: u64) {
    unsafe { sys::ecs_remove_id(w, e, id) }
}

pub fn alive(w: RawWorld, e: u64) -> bool {
    e != 0 && unsafe { sys::ecs_is_alive(w, e) }
}

pub fn deferred(w: RawWorld) -> bool {
    unsafe { sys::ecs_is_deferred(w) }
}

pub fn enabled(w: RawWorld, e: u64) -> bool {
    !has(w, e, unsafe { sys::EcsDisabled })
}

/// Size of a component id (0 for tags and non-components).
pub fn type_size(w: RawWorld, id: u64) -> usize {
    let ti = unsafe { sys::ecs_get_type_info(w, id) };
    if ti.is_null() {
        0
    } else {
        unsafe { (*ti).size as usize }
    }
}

pub fn entity_ids(w: RawWorld, e: u64) -> Vec<u64> {
    let t = unsafe { sys::ecs_get_type(w, e) };
    if t.is_null() {
        return vec![];
    }
    let t = unsafe { &*t };
    unsafe { std::slice::from_raw_parts(t.array, t.count as usize) }.to_vec()
}

pub fn builtin_phase(name: &str) -> Option<u64> {
    unsafe {
        Some(match name {
            "OnLoad" => sys::EcsOnLoad,
            "PostLoad" => sys::EcsPostLoad,
            "PreUpdate" => sys::EcsPreUpdate,
            "OnUpdate" => sys::EcsOnUpdate,
            "OnValidate" => sys::EcsOnValidate,
            "PostUpdate" => sys::EcsPostUpdate,
            "PreStore" => sys::EcsPreStore,
            "OnStore" => sys::EcsOnStore,
            _ => return None,
        })
    }
}

pub fn builtin_event(name: &str) -> Option<u64> {
    unsafe {
        Some(match name {
            "OnAdd" => sys::EcsOnAdd,
            "OnSet" => sys::EcsOnSet,
            "OnRemove" => sys::EcsOnRemove,
            _ => return None,
        })
    }
}
