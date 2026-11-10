//! Step 0: the Flecs behaviors the Python binding is built on, checked at the
//! pinned version (flecs_ecs 0.2.2, Flecs C 4.1.2) through the C API, the
//! same way the binding uses it.

use std::ffi::{CString, c_void};

use flecs_ecs::prelude::*;
use flecs_ecs::sys;

fn cstr(s: &str) -> CString {
    CString::new(s).unwrap()
}

unsafe fn named(w: *mut sys::ecs_world_t, parent: u64, name: &str) -> u64 {
    let name = cstr(name);
    let desc = sys::ecs_entity_desc_t {
        name: name.as_ptr(),
        parent,
        sep: c"::".as_ptr(),
        ..Default::default()
    };
    unsafe { sys::ecs_entity_init(w, &desc) }
}

unsafe fn struct_type(
    w: *mut sys::ecs_world_t,
    parent: u64,
    name: &str,
    members: &[(&str, u64)],
) -> u64 {
    unsafe {
        let e = named(w, parent, name);
        let names: Vec<CString> = members.iter().map(|m| cstr(m.0)).collect();
        let mut desc: sys::ecs_struct_desc_t = std::mem::zeroed();
        desc.entity = e;
        for (i, (_, ty)) in members.iter().enumerate() {
            desc.members[i].name = names[i].as_ptr();
            desc.members[i].type_ = *ty;
        }
        let id = sys::ecs_struct_init(w, &desc);
        assert_eq!(id, e);
        id
    }
}

fn component_info(w: *mut sys::ecs_world_t, id: u64) -> sys::EcsComponent {
    unsafe {
        let p = sys::ecs_get_id(w, id, sys::FLECS_IDEcsComponentID_) as *const sys::EcsComponent;
        assert!(!p.is_null());
        *p
    }
}

fn member_offsets(w: *mut sys::ecs_world_t, id: u64) -> Vec<(String, i32)> {
    unsafe {
        let s = sys::ecs_get_id(w, id, sys::FLECS_IDEcsStructID_) as *const sys::EcsStruct;
        assert!(!s.is_null());
        let v = &(*s).members;
        let ms = std::slice::from_raw_parts(v.array as *const sys::ecs_member_t, v.count as usize);
        ms.iter()
            .map(|m| {
                (
                    std::ffi::CStr::from_ptr(m.name)
                        .to_string_lossy()
                        .into_owned(),
                    m.offset,
                )
            })
            .collect()
    }
}

unsafe fn query(w: *mut sys::ecs_world_t, expr: &str) -> *mut sys::ecs_query_t {
    let expr = cstr(expr);
    let mut desc: sys::ecs_query_desc_t = unsafe { std::mem::zeroed() };
    desc.expr = expr.as_ptr();
    unsafe { sys::ecs_query_init(w, &desc) }
}

#[test]
fn runtime_struct_has_c_layout() {
    let world = World::new();
    let w = world.ptr_mut();
    unsafe {
        let t = struct_type(
            w,
            0,
            "Mixed",
            &[
                ("a", sys::FLECS_IDecs_f32_tID_),
                ("b", sys::FLECS_IDecs_f64_tID_),
                ("c", sys::FLECS_IDecs_bool_tID_),
                ("d", sys::FLECS_IDecs_u64_tID_),
            ],
        );
        let info = component_info(w, t);
        assert_eq!((info.size, info.alignment), (32, 8));
        assert_eq!(
            member_offsets(w, t),
            vec![
                ("a".into(), 0),
                ("b".into(), 8),
                ("c".into(), 16),
                ("d".into(), 24)
            ]
        );
    }
}

#[test]
fn inheritable_base_matches_derived_and_reports_derived_id() {
    let world = World::new();
    let w = world.ptr_mut();
    unsafe {
        let base = struct_type(w, 0, "Base", &[("x", sys::FLECS_IDecs_f32_tID_)]);
        sys::ecs_add_id(w, base, sys::EcsInheritable);
        let derived = struct_type(
            w,
            0,
            "Derived",
            &[
                ("x", sys::FLECS_IDecs_f32_tID_),
                ("y", sys::FLECS_IDecs_f32_tID_),
            ],
        );
        sys::ecs_add_id(w, derived, ecs_pair(sys::EcsIsA, base));

        let e1 = sys::ecs_new(w);
        let v1: [f32; 1] = [1.0];
        sys::ecs_set_id(w, e1, base, 4, v1.as_ptr() as *const c_void);
        let e2 = sys::ecs_new(w);
        let v2: [f32; 2] = [2.0, 3.0];
        sys::ecs_set_id(w, e2, derived, 8, v2.as_ptr() as *const c_void);
        let e3 = sys::ecs_new(w);
        let v3: [f32; 2] = [4.0, 5.0];
        sys::ecs_set_id(w, e3, derived, 8, v3.as_ptr() as *const c_void);

        let q = query(w, "Base");
        assert!(!q.is_null());
        let mut it = sys::ecs_query_iter(w, q);
        let mut seen = vec![];
        while sys::ecs_query_next(&mut it) {
            let id = sys::ecs_field_id(&it, 0);
            // Finding: ecs_field_size reports the *term's* size (Base, 4),
            // not the matched column's. The stride must come from the type
            // info of the matched id (Derived, 8).
            assert_eq!(sys::ecs_field_size(&it, 0), 4);
            let stride = (*sys::ecs_get_type_info(w, id)).size as usize;
            let ptr = sys::ecs_field_w_size(&it, 4, 0) as *const u8;
            for row in 0..it.count as usize {
                let first = *ptr.add(row * stride).cast::<f32>();
                seen.push((*it.entities.add(row), id, stride, first));
            }
        }
        seen.sort_by_key(|s| s.0);
        assert_eq!(
            seen,
            vec![
                (e1, base, 4, 1.0),
                (e2, derived, 8, 2.0),
                (e3, derived, 8, 4.0)
            ]
        );
        sys::ecs_query_fini(q);
    }
}

#[test]
fn expr_resolves_names_in_scope_with_traversal_optional_and_not() {
    let world = World::new();
    let w = world.ptr_mut();
    unsafe {
        let module = named(w, 0, "game");
        sys::ecs_add_id(w, module, sys::EcsModule);
        let pos = struct_type(w, module, "Pos", &[("x", sys::FLECS_IDecs_f32_tID_)]);
        let vel = struct_type(w, module, "Vel", &[("x", sys::FLECS_IDecs_f32_tID_)]);
        let frozen = named(w, module, "Frozen");
        let _ = vel;

        let parent = sys::ecs_new(w);
        let pv: f32 = 10.0;
        sys::ecs_set_id(w, parent, pos, 4, &pv as *const f32 as *const c_void);
        let child = sys::ecs_new_w_id(w, ecs_pair(sys::EcsChildOf, parent));
        let cv: f32 = 1.0;
        sys::ecs_set_id(w, child, pos, 4, &cv as *const f32 as *const c_void);
        let frozen_child = sys::ecs_new_w_id(w, ecs_pair(sys::EcsChildOf, parent));
        sys::ecs_set_id(w, frozen_child, pos, 4, &cv as *const f32 as *const c_void);
        sys::ecs_add_id(w, frozen_child, frozen);

        let old = sys::ecs_set_scope(w, module);
        let q = query(w, "Pos, ?Vel, !Frozen, Pos(up ChildOf)");
        sys::ecs_set_scope(w, old);
        assert!(!q.is_null(), "query with scoped names failed to parse");
        assert_eq!((*q).field_count, 4);
        let mut it = sys::ecs_query_iter(w, q);
        let mut rows = vec![];
        while sys::ecs_query_next(&mut it) {
            assert!(!sys::ecs_field_is_set(&it, 1), "optional Vel is absent");
            assert!(!sys::ecs_field_is_self(&it, 3), "up field is shared");
            let up = *(sys::ecs_field_w_size(&it, 4, 3) as *const f32);
            for row in 0..it.count as usize {
                rows.push((*it.entities.add(row), up));
            }
        }
        assert_eq!(rows, vec![(child, 10.0)]);
        sys::ecs_query_fini(q);
    }
}

thread_local! {
    static ORDER: std::cell::RefCell<Vec<u64>> = const { std::cell::RefCell::new(Vec::new()) };
}

unsafe extern "C-unwind" fn cmp_u64(
    _e1: u64,
    p1: *const c_void,
    _e2: u64,
    p2: *const c_void,
) -> i32 {
    let (a, b) = unsafe { (*(p1 as *const u64), *(p2 as *const u64)) };
    a.cmp(&b) as i32
}

unsafe extern "C-unwind" fn record(it: *mut sys::ecs_iter_t) {
    unsafe {
        let it = &*it;
        let p = sys::ecs_field_w_size(it, 8, 0) as *const u64;
        for row in 0..it.count as usize {
            ORDER.with(|o| o.borrow_mut().push(*p.add(row)));
        }
    }
}

#[test]
fn system_order_by_runtime_component_sorts_across_tables() {
    let world = World::new();
    let w = world.ptr_mut();
    unsafe {
        let sid = struct_type(w, 0, "Sid", &[("value", sys::FLECS_IDecs_u64_tID_)]);
        let tag_a = named(w, 0, "TagA");
        let tag_b = named(w, 0, "TagB");
        // Spread over three tables, created out of order.
        for (v, tag) in [
            (5u64, tag_a),
            (1, tag_b),
            (4, 0),
            (2, tag_a),
            (3, tag_b),
            (0, 0),
        ] {
            let e = sys::ecs_new(w);
            sys::ecs_set_id(w, e, sid, 8, &v as *const u64 as *const c_void);
            if tag != 0 {
                sys::ecs_add_id(w, e, tag);
            }
        }
        let sys_e = named(w, 0, "Ordered");
        sys::ecs_add_id(w, sys_e, ecs_pair(sys::EcsDependsOn, sys::EcsOnUpdate));
        sys::ecs_add_id(w, sys_e, sys::EcsOnUpdate);
        let expr = cstr("Sid");
        let mut desc: sys::ecs_system_desc_t = std::mem::zeroed();
        desc.entity = sys_e;
        desc.query.expr = expr.as_ptr();
        desc.query.order_by = sid;
        desc.query.order_by_callback = Some(cmp_u64);
        desc.callback = Some(record);
        assert_eq!(sys::ecs_system_init(w, &desc), sys_e);
        world.progress();
        assert_eq!(ORDER.with(|o| o.borrow().clone()), vec![0, 1, 2, 3, 4, 5]);
    }
}

fn matches(w: *mut sys::ecs_world_t, q: *mut sys::ecs_query_t) -> Vec<u64> {
    let mut out = vec![];
    unsafe {
        let mut it = sys::ecs_query_iter(w, q);
        while sys::ecs_query_next(&mut it) {
            for row in 0..it.count as usize {
                out.push(*it.entities.add(row));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn isa_term_matches_instances_of_derived_prefabs_but_not_prefabs() {
    // Design doc 13.5: "Verify that IsA matching covers instances of derived
    // prefabs (5.4)". IsA is Transitive at this pin.
    let world = World::new();
    let w = world.ptr_mut();
    unsafe {
        let enemy = named(w, 0, "Enemy");
        sys::ecs_add_id(w, enemy, sys::EcsPrefab);
        let boss = named(w, 0, "Boss");
        sys::ecs_add_id(w, boss, sys::EcsPrefab);
        sys::ecs_add_id(w, boss, ecs_pair(sys::EcsIsA, enemy));
        let goblin = sys::ecs_new_w_id(w, ecs_pair(sys::EcsIsA, enemy));
        let dragon = sys::ecs_new_w_id(w, ecs_pair(sys::EcsIsA, boss));
        let _plain = sys::ecs_new(w);
        sys::ecs_add_id(w, _plain, sys::EcsModule);

        let q = query(w, "(IsA, Enemy)");
        assert!(!q.is_null());
        let mut want = vec![goblin, dragon];
        want.sort();
        assert_eq!(
            matches(w, q),
            want,
            "instances of Enemy and of Boss (IsA Enemy)"
        );
        sys::ecs_query_fini(q);
    }
}

#[test]
fn instances_copy_components_by_default_and_dont_inherit_is_respected() {
    let world = World::new();
    let w = world.ptr_mut();
    unsafe {
        let hp = struct_type(w, 0, "Hp", &[("v", sys::FLECS_IDecs_i64_tID_)]);
        let sid = struct_type(w, 0, "Sid", &[("value", sys::FLECS_IDecs_u64_tID_)]);
        sys::ecs_add_id(w, sid, ecs_pair(sys::EcsOnInstantiate, sys::EcsDontInherit));
        let enemy = named(w, 0, "Enemy");
        sys::ecs_add_id(w, enemy, sys::EcsPrefab);
        let ten: i64 = 10;
        sys::ecs_set_id(w, enemy, hp, 8, &ten as *const i64 as *const c_void);
        let seven: u64 = 7;
        sys::ecs_set_id(w, enemy, sid, 8, &seven as *const u64 as *const c_void);

        let inst = sys::ecs_new_w_id(w, ecs_pair(sys::EcsIsA, enemy));
        // Default (Override): the instance owns a copy it can write.
        let p = sys::ecs_get_mut_id(w, inst, hp) as *mut i64;
        assert!(!p.is_null() && *p == 10);
        *p = 3;
        assert_eq!(*(sys::ecs_get_id(w, enemy, hp) as *const i64), 10);
        // DontInherit: the prefab's StableId is not copied or visible.
        assert!(!sys::ecs_has_id(w, inst, sid));
    }
}

#[test]
fn optional_prefab_and_disabled_terms_include_them() {
    let world = World::new();
    let w = world.ptr_mut();
    unsafe {
        let tag = named(w, 0, "T");
        let prefab = sys::ecs_new_w_id(w, tag);
        sys::ecs_add_id(w, prefab, sys::EcsPrefab);
        let off = sys::ecs_new_w_id(w, tag);
        sys::ecs_enable(w, off, false);
        let on = sys::ecs_new_w_id(w, tag);

        let mut desc: sys::ecs_query_desc_t = std::mem::zeroed();
        desc.terms[0].id = tag;
        let q = sys::ecs_query_init(w, &desc);
        assert_eq!(
            matches(w, q),
            vec![on],
            "default skips prefabs and disabled"
        );
        sys::ecs_query_fini(q);
        // Finding: setting EcsQueryMatchPrefab | EcsQueryMatchDisabled in
        // ecs_query_desc_t::flags did not include them at this pin; naming the
        // tags as optional terms does.
        desc.terms[1].id = sys::EcsPrefab;
        desc.terms[1].oper = sys::ecs_oper_kind_t_EcsOptional as i16;
        desc.terms[2].id = sys::EcsDisabled;
        desc.terms[2].oper = sys::ecs_oper_kind_t_EcsOptional as i16;
        let q = sys::ecs_query_init(w, &desc);
        let mut all = vec![prefab, off, on];
        all.sort();
        assert_eq!(matches(w, q), all);
        sys::ecs_query_fini(q);
    }
}
