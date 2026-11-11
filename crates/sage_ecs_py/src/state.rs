//! Generic world-state hashing, snapshots and restore, driven by reflection.
//!
//! Scope: every entity with a StableId (prefabs and disabled entities
//! included), every component the binding knows (Python-defined and
//! registered native ones), relationships whose relationship entity is a
//! known component, `ChildOf`, `IsA`, the `Prefab` and `Disabled` tags, every singleton of a
//! known component, the enabled state of flecs modules created through the
//! binding, the StableId counter and the world RNG. Native state the binding
//! does not know (for example a contact table) is the host's to hash.
//!
//! Values are fed field by field, never as raw bytes (padding is not
//! stable), and entity references as the target's StableId (Flecs ids are
//! not stable across runs). Opaque (Python object) components are skipped:
//! hashing and snapshotting them is an [Open] item in the design doc.

use std::collections::BTreeMap;
use std::ffi::c_void;

use flecs_ecs::sys;
use sage_core::{Rng, StateHasher};
use sage_scene::StringTable;

use crate::ffi;
use crate::registry::{FieldInfo, Kind, Layout, Registry};

/// One component's value with entity references replaced by StableIds.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredValue {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntitySnapshot {
    pub sid: u64,
    pub name: Option<String>,
    pub values: Vec<StoredValue>,
    pub tags: Vec<String>,
    /// (relationship path, target StableId or 0 if the target has none,
    /// target path when it has no StableId)
    pub pairs: Vec<(String, u64, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub entities: Vec<EntitySnapshot>,
    pub singletons: Vec<StoredValue>,
    pub modules: Vec<(String, bool)>,
    pub next_sid: u64,
    pub rng: Rng,
    pub strings: StringTable,
}

/// Built-in Flecs relationships that are simulation state.
fn builtin_rels() -> [(&'static str, u64); 2] {
    unsafe {
        [
            ("flecs:ChildOf", sys::EcsChildOf),
            ("flecs:IsA", sys::EcsIsA),
        ]
    }
}

/// Built-in Flecs tags that are simulation state.
fn builtin_tags() -> [(&'static str, u64); 2] {
    unsafe {
        [
            ("flecs:Prefab", sys::EcsPrefab),
            ("flecs:Disabled", sys::EcsDisabled),
        ]
    }
}

fn builtin_name(table: &[(&'static str, u64)], id: u64) -> Option<&'static str> {
    table.iter().find(|(_, i)| *i == id).map(|(n, _)| *n)
}

fn builtin_id(table: &[(&'static str, u64)], name: &str) -> Option<u64> {
    table.iter().find(|(n, _)| *n == name).map(|(_, i)| *i)
}

enum Item {
    Value(String, u64),
    Tag(String),
    Pair(String, u64, String),
}

/// What of `e` is in scope, sorted by path.
fn items(reg: &Registry, e: u64) -> Vec<Item> {
    let w = reg.world;
    let types = reg.types.borrow();
    let rels = builtin_rels();
    let tags = builtin_tags();
    let mut out = vec![];
    for id in ffi::entity_ids(w, e) {
        if ffi::is_pair(id) {
            let rel = ffi::pair_first(w, id);
            let builtin = builtin_name(&rels, rel);
            if builtin.is_some() || types.contains_key(&rel) {
                let tgt = ffi::pair_second(w, id);
                let rel_path = match builtin {
                    Some(n) => n.to_string(),
                    None => types[&rel].path.clone(),
                };
                let sid = reg.sid_of(tgt).unwrap_or(0);
                let tpath = if sid == 0 {
                    ffi::path(w, tgt)
                } else {
                    String::new()
                };
                out.push(Item::Pair(rel_path, sid, tpath));
            }
            continue;
        }
        if id == reg.sid_type {
            continue;
        }
        if let Some(n) = builtin_name(&tags, id) {
            out.push(Item::Tag(n.to_string()));
            continue;
        }
        let Some(t) = types.get(&id) else { continue };
        match t.layout {
            Layout::Struct(_) => out.push(Item::Value(t.path.clone(), id)),
            Layout::Tag => out.push(Item::Tag(t.path.clone())),
            Layout::Opaque => {}
        }
    }
    out.sort_by(|a, b| key(a).cmp(&key(b)));
    out
}

fn key(i: &Item) -> (u8, &str, u64, &str) {
    match i {
        Item::Value(p, _) => (0, p, 0, ""),
        Item::Tag(p) => (1, p, 0, ""),
        Item::Pair(p, s, t) => (2, p, *s, t),
    }
}

fn fields_of(reg: &Registry, id: u64) -> Vec<FieldInfo> {
    match &reg.types.borrow()[&id].layout {
        Layout::Struct(f) => f.clone(),
        _ => vec![],
    }
}

fn hash_fields(reg: &Registry, h: &mut StateHasher, fields: &[FieldInfo], p: *const u8) {
    for f in fields {
        let q = unsafe { p.add(f.offset) };
        unsafe {
            match f.kind {
                Kind::Bool | Kind::U8 => h.u8(*q),
                Kind::I32 | Kind::U32 | Kind::F32 => h.u32(*(q as *const u32)),
                Kind::I64 | Kind::U64 | Kind::F64 => h.u64(*(q as *const u64)),
                Kind::Entity => h.u64(reg.sid_of(*(q as *const u64)).unwrap_or(0)),
            };
        }
    }
}

/// Entities with a StableId, in StableId order, including prefabs and
/// disabled entities. Rebuilds the index from the world, so entities given a
/// StableId natively are included.
pub fn entities(reg: &Registry) -> Vec<(u64, u64)> {
    let w = reg.world;
    let mut desc: sys::ecs_query_desc_t = unsafe { std::mem::zeroed() };
    desc.terms[0].id = reg.sid_type;
    // Queries skip prefabs and disabled entities unless these terms are
    // named (the query flags do not do it at this pin: flecs_assumptions.rs).
    unsafe {
        desc.terms[1].id = sys::EcsPrefab;
        desc.terms[1].oper = sys::ecs_oper_kind_t_EcsOptional as i16;
        desc.terms[2].id = sys::EcsDisabled;
        desc.terms[2].oper = sys::ecs_oper_kind_t_EcsOptional as i16;
    }
    let q = unsafe { sys::ecs_query_init(w, &desc) };
    let mut out = BTreeMap::new();
    unsafe {
        let mut it = sys::ecs_query_iter(w, q);
        while sys::ecs_query_next(&mut it) {
            let p = sys::ecs_field_w_size(&it, 8, 0) as *const u64;
            for row in 0..it.count as usize {
                out.insert(*p.add(row), *it.entities.add(row));
            }
        }
        sys::ecs_query_fini(q);
    }
    *reg.sids.borrow_mut() = out.clone();
    out.into_iter().collect()
}

pub fn hash_into(reg: &Registry, h: &mut StateHasher) {
    let w = reg.world;
    for (sid, e) in entities(reg) {
        h.tag("entity").u64(sid);
        for item in items(reg, e) {
            match item {
                Item::Value(path, id) => {
                    h.tag(&path);
                    let p = unsafe { sys::ecs_get_id(w, e, id) } as *const u8;
                    hash_fields(reg, h, &fields_of(reg, id), p);
                }
                Item::Tag(path) => {
                    h.tag("tag").tag(&path);
                }
                Item::Pair(rel, sid, tpath) => {
                    h.tag("pair").tag(&rel).u64(sid).tag(&tpath);
                }
            }
        }
    }
    let singletons: Vec<(String, u64)> = reg
        .types
        .borrow()
        .iter()
        .filter(|(id, t)| matches!(t.layout, Layout::Struct(_)) && ffi::has(w, **id, **id))
        .map(|(id, t)| (t.path.clone(), *id))
        .collect();
    let mut singletons = singletons;
    singletons.sort();
    for (path, id) in singletons {
        h.tag("singleton").tag(&path);
        let p = unsafe { sys::ecs_get_id(w, id, id) } as *const u8;
        hash_fields(reg, h, &fields_of(reg, id), p);
    }
    for (path, enabled) in modules(reg) {
        h.tag("module").tag(&path).u8(enabled as u8);
    }
    h.tag("next_sid").u64(reg.next_sid.get());
    for s in reg.rng.borrow().state() {
        h.tag("rng").u64(s);
    }
    // Only when used, so a world without text hashes as it always did.
    let strings = reg.strings.borrow();
    if !strings.is_empty() {
        h.tag("strings");
        for (id, text) in strings.iter() {
            h.u64(id).u64(text.len() as u64).bytes(text.as_bytes());
        }
    }
}

fn modules(reg: &Registry) -> Vec<(String, bool)> {
    let w = reg.world;
    let mut m: Vec<_> = reg
        .modules
        .borrow()
        .iter()
        .filter(|e| ffi::alive(w, **e))
        .map(|e| (ffi::path(w, *e), ffi::enabled(w, *e)))
        .collect();
    m.sort();
    m
}

fn store(reg: &Registry, id: u64, p: *const u8) -> StoredValue {
    let t = &reg.types.borrow()[&id];
    let mut bytes = unsafe { std::slice::from_raw_parts(p, t.size) }.to_vec();
    if let Layout::Struct(fields) = &t.layout {
        for f in fields.iter().filter(|f| f.kind == Kind::Entity) {
            let raw = u64::from_ne_bytes(bytes[f.offset..f.offset + 8].try_into().unwrap());
            let sid = reg.sid_of(raw).unwrap_or(0);
            bytes[f.offset..f.offset + 8].copy_from_slice(&sid.to_ne_bytes());
        }
    }
    StoredValue {
        path: t.path.clone(),
        bytes,
    }
}

pub fn snapshot(reg: &Registry) -> Snapshot {
    let w = reg.world;
    let mut entities_out = vec![];
    for (sid, e) in entities(reg) {
        let mut es = EntitySnapshot {
            sid,
            name: ffi::name(w, e),
            values: vec![],
            tags: vec![],
            pairs: vec![],
        };
        for item in items(reg, e) {
            match item {
                Item::Value(_, id) => {
                    let p = unsafe { sys::ecs_get_id(w, e, id) } as *const u8;
                    es.values.push(store(reg, id, p));
                }
                Item::Tag(path) => es.tags.push(path),
                Item::Pair(rel, sid, tpath) => es.pairs.push((rel, sid, tpath)),
            }
        }
        entities_out.push(es);
    }
    let ids: Vec<u64> = reg
        .types
        .borrow()
        .iter()
        .filter(|(id, t)| matches!(t.layout, Layout::Struct(_)) && ffi::has(w, **id, **id))
        .map(|(id, _)| *id)
        .collect();
    let mut singletons: Vec<_> = ids
        .into_iter()
        .map(|id| store(reg, id, unsafe { sys::ecs_get_id(w, id, id) } as *const u8))
        .collect();
    singletons.sort_by(|a, b| a.path.cmp(&b.path));
    Snapshot {
        entities: entities_out,
        singletons,
        modules: modules(reg),
        next_sid: reg.next_sid.get(),
        rng: reg.rng.borrow().clone(),
        strings: reg.strings.borrow().clone(),
    }
}

pub(crate) fn id_by_path(reg: &Registry) -> BTreeMap<String, u64> {
    reg.types
        .borrow()
        .iter()
        .map(|(id, t)| (t.path.clone(), *id))
        .collect()
}

fn write_value(reg: &Registry, e: u64, id: u64, v: &StoredValue) -> Result<(), String> {
    let w = reg.world;
    let t_size = reg.types.borrow()[&id].size;
    if t_size != v.bytes.len() {
        return Err(format!(
            "{}: size changed ({} -> {t_size})",
            v.path,
            v.bytes.len()
        ));
    }
    let mut bytes = v.bytes.clone();
    for f in fields_of(reg, id).iter().filter(|f| f.kind == Kind::Entity) {
        let sid = u64::from_ne_bytes(bytes[f.offset..f.offset + 8].try_into().unwrap());
        let raw = if sid == 0 {
            0
        } else {
            reg.entity_of_sid(sid).unwrap_or(0)
        };
        bytes[f.offset..f.offset + 8].copy_from_slice(&raw.to_ne_bytes());
    }
    // Copy into aligned storage before handing it to Flecs.
    let mut aligned = vec![0u64; bytes.len().div_ceil(8).max(1)];
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), aligned.as_mut_ptr() as *mut u8, bytes.len());
        sys::ecs_set_id(w, e, id, bytes.len(), aligned.as_ptr() as *const c_void);
    }
    Ok(())
}

/// Restores `s` into this world, which must have loaded the same script
/// (so the same component types exist). Entities are matched by StableId:
/// missing ones are created, extra ones deleted.
pub fn restore(reg: &Registry, s: &Snapshot) -> Result<(), String> {
    let w = reg.world;
    if ffi::deferred(w) {
        return Err("cannot restore while the world is running a tick".into());
    }
    let by_path = id_by_path(reg);
    let lookup = |path: &str| -> Result<u64, String> {
        by_path
            .get(path)
            .copied()
            .ok_or_else(|| format!("snapshot has unknown component {path}"))
    };
    let wanted: BTreeMap<u64, &EntitySnapshot> = s.entities.iter().map(|e| (e.sid, e)).collect();
    // Delete extra entities, create missing ones (relationships need all).
    for (sid, e) in entities(reg) {
        if !wanted.contains_key(&sid) {
            unsafe { sys::ecs_delete(w, e) };
        }
    }
    reg.sids
        .borrow_mut()
        .retain(|sid, _| wanted.contains_key(sid));
    for es in &s.entities {
        if reg.entity_of_sid(es.sid).is_none() {
            let e = match &es.name {
                Some(n) if ffi::lookup_child(w, 0, n) == 0 => ffi::named_child(w, 0, n),
                _ => unsafe { sys::ecs_new(w) },
            };
            reg.assign_sid(e, Some(es.sid))?;
        }
    }
    let rels = builtin_rels();
    let tags = builtin_tags();
    for es in &s.entities {
        let e = reg.entity_of_sid(es.sid).expect("created above");
        let mut keep = std::collections::BTreeSet::new();
        // Tags and relationships first: adding (IsA, prefab) copies the
        // prefab's components, which the values below then overwrite.
        for t in &es.tags {
            let id = match builtin_id(&tags, t) {
                Some(id) => id,
                None => lookup(t)?,
            };
            ffi::add(w, e, id);
            keep.insert(id);
        }
        for (rel, sid, tpath) in &es.pairs {
            let r = match builtin_id(&rels, rel) {
                Some(id) => id,
                None => lookup(rel)?,
            };
            let tgt = if *sid != 0 {
                reg.entity_of_sid(*sid)
                    .ok_or_else(|| format!("pair target StableId {sid} missing"))?
            } else {
                let t = ffi::lookup_path(w, tpath);
                if t == 0 {
                    return Err(format!("pair target {tpath} missing"));
                }
                t
            };
            let id = ffi::pair(r, tgt);
            ffi::add(w, e, id);
            keep.insert(id);
        }
        for v in &es.values {
            let id = lookup(&v.path)?;
            write_value(reg, e, id, v)?;
            keep.insert(id);
        }
        // Remove what the snapshot does not have.
        let types = reg.types.borrow();
        for id in ffi::entity_ids(w, e) {
            if keep.contains(&id) || id == reg.sid_type {
                continue;
            }
            let known = if ffi::is_pair(id) {
                let rel = ffi::pair_first(w, id);
                builtin_name(&rels, rel).is_some() || types.contains_key(&rel)
            } else {
                builtin_name(&tags, id).is_some()
                    || types
                        .get(&id)
                        .is_some_and(|t| !matches!(t.layout, Layout::Opaque))
            };
            if known {
                ffi::remove(w, e, id);
            }
        }
    }
    let present: Vec<u64> = reg
        .types
        .borrow()
        .iter()
        .filter(|(id, t)| matches!(t.layout, Layout::Struct(_)) && ffi::has(w, **id, **id))
        .map(|(id, _)| *id)
        .collect();
    let wanted_singletons: BTreeMap<&str, &StoredValue> =
        s.singletons.iter().map(|v| (v.path.as_str(), v)).collect();
    for id in present {
        let path = reg.types.borrow()[&id].path.clone();
        if !wanted_singletons.contains_key(path.as_str()) {
            ffi::remove(w, id, id);
        }
    }
    for v in &s.singletons {
        let id = lookup(&v.path)?;
        write_value(reg, id, id, v)?;
    }
    let mods: BTreeMap<String, u64> = reg
        .modules
        .borrow()
        .iter()
        .map(|e| (ffi::path(w, *e), *e))
        .collect();
    for (path, enabled) in &s.modules {
        let e = mods
            .get(path)
            .ok_or_else(|| format!("snapshot has unknown flecs module {path}"))?;
        unsafe { sys::ecs_enable(w, *e, *enabled) };
    }
    reg.next_sid.set(s.next_sid);
    *reg.rng.borrow_mut() = s.rng.clone();
    *reg.strings.borrow_mut() = s.strings.clone();
    reg.invalidate_views();
    Ok(())
}

/// True if any entity with a StableId carries an opaque (Python object)
/// component, which `hash_into` and `snapshot` skip.
pub fn has_opaque_state(reg: &Registry) -> bool {
    let w = reg.world;
    let types = reg.types.borrow();
    entities(reg).into_iter().any(|(_, e)| {
        ffi::entity_ids(w, e).into_iter().any(|id| {
            types
                .get(&id)
                .is_some_and(|t| matches!(t.layout, Layout::Opaque))
        })
    })
}
