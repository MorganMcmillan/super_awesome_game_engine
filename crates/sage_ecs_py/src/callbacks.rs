//! Systems and observers whose bodies are Python functions.
//!
//! The Python side (`sage.ecs`) turns a function signature into a query
//! expression plus a parameter plan. Here the query is built, the plan is
//! checked against the fields Flecs reports, and the function is called once
//! per matched entity.
//!
//! Systems carry an implicit first term, `sage.StableId`, and are ordered by
//! it, so per-entity calls happen in StableId order: the same order after a
//! snapshot is restored into a fresh world, where Flecs's table order would
//! differ (design doc 3.3).

use std::ffi::c_void;
use std::rc::{Rc, Weak};

use flecs_ecs::sys;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::api::{Entity, class_for, object_at};
use crate::ffi;
use crate::registry::{Layout, Registry};
use crate::value;

pub struct Spec<'a> {
    pub kind: &'a str,
    pub entity: u64,
    pub phase: u64,
    pub expr: &'a str,
    /// Number of terms placed by the binding before the user's `query=`.
    pub n_fixed: usize,
    /// (kind, field index or None for "next data field of query=", name)
    pub params: Vec<(String, Option<usize>, String)>,
    pub func: Py<PyAny>,
    pub immediate: bool,
    pub events: Vec<u64>,
    pub event_type: u64,
}

#[derive(Clone, Copy, Debug)]
enum Param {
    Entity,
    Field { index: i8, readonly: bool },
    Event,
}

struct Ctx {
    reg: Weak<Registry>,
    func: Py<PyAny>,
    params: Vec<Param>,
    event_type: u64,
    label: String,
    observer: bool,
    /// (event_cur, entity) of the last event handled: an `_` (Any) term
    /// matches each id the event carries, so one emit can arrive twice.
    last_event: std::cell::Cell<(i32, u64)>,
}

unsafe extern "C-unwind" fn free_ctx(p: *mut c_void) {
    if p.is_null() {
        return;
    }
    let ctx = unsafe { Box::from_raw(p as *mut Ctx) };
    Python::attach(|_py| drop(ctx));
}

unsafe extern "C-unwind" fn cmp_sid(
    _e1: u64,
    p1: *const c_void,
    _e2: u64,
    p2: *const c_void,
) -> i32 {
    let (a, b) = unsafe { (*(p1 as *const u64), *(p2 as *const u64)) };
    a.cmp(&b) as i32
}

/// Field facts of a parsed query: (field count, data fields, write fields).
fn inspect(w: ffi::RawWorld, expr: &str) -> Result<(usize, u32, u32), String> {
    let cexpr = ffi::cstring(expr);
    let mut desc: sys::ecs_query_desc_t = unsafe { std::mem::zeroed() };
    desc.expr = cexpr.as_ptr();
    let q = unsafe { sys::ecs_query_init(w, &desc) };
    if q.is_null() {
        return Err(format!(
            "invalid query {expr:?} (Flecs printed the reason above)"
        ));
    }
    let facts = unsafe {
        (
            (*q).field_count as usize,
            (*q).data_fields,
            (*q).write_fields,
        )
    };
    unsafe { sys::ecs_query_fini(q) };
    Ok(facts)
}

pub fn create(reg: &Rc<Registry>, spec: Spec<'_>) -> Result<(), String> {
    let w = reg.world;
    let observer = spec.kind == "observer";
    let label = format!("{} {}", spec.kind, ffi::path(w, spec.entity));

    // Names in queries resolve relative to the script's flecs module.
    let old_scope = unsafe { sys::ecs_set_scope(w, reg.script_module.get()) };
    let result = (|| {
        let (field_count, data, write) = inspect(w, spec.expr)?;
        let mut next_free = spec.n_fixed;
        let mut params = vec![];
        for (kind, index, pname) in &spec.params {
            params.push(match kind.as_str() {
                "entity" => Param::Entity,
                "event" => Param::Event,
                "field" => {
                    let index = match index {
                        Some(i) => *i,
                        None => {
                            while next_free < field_count && data & (1 << next_free) == 0 {
                                next_free += 1;
                            }
                            if next_free >= field_count {
                                return Err(format!(
                                    "{label}: parameter {pname:?} has no annotation and there \
                                     is no data term left in query= to bind it to \
                                     (tags, `!` and filter terms carry no data)"
                                ));
                            }
                            next_free += 1;
                            next_free - 1
                        }
                    };
                    if index >= field_count || data & (1 << index) == 0 {
                        return Err(format!(
                            "{label}: parameter {pname:?} is bound to a term without data"
                        ));
                    }
                    Param::Field {
                        index: index as i8,
                        readonly: write & (1 << index) == 0,
                    }
                }
                other => return Err(format!("{label}: unknown parameter kind {other}")),
            });
        }

        let ctx = Box::into_raw(Box::new(Ctx {
            reg: Rc::downgrade(reg),
            func: Python::attach(|py| spec.func.clone_ref(py)),
            params,
            event_type: spec.event_type,
            label: label.clone(),
            observer,
            last_event: std::cell::Cell::new((-1, 0)),
        })) as *mut c_void;
        let cexpr = ffi::cstring(spec.expr);
        let created = if observer {
            if spec.events.is_empty() || spec.events.len() > 8 {
                unsafe { free_ctx(ctx) };
                return Err(format!("{label}: an observer needs 1 to 8 events"));
            }
            let mut desc: sys::ecs_observer_desc_t = unsafe { std::mem::zeroed() };
            desc.entity = spec.entity;
            desc.query.expr = cexpr.as_ptr();
            for (i, ev) in spec.events.iter().enumerate() {
                desc.events[i] = *ev;
            }
            desc.callback = Some(run);
            desc.callback_ctx = ctx;
            desc.callback_ctx_free = Some(free_ctx);
            unsafe { sys::ecs_observer_init(w, &desc) }
        } else {
            ffi::add(
                w,
                spec.entity,
                ffi::pair(unsafe { sys::EcsDependsOn }, spec.phase),
            );
            ffi::add(w, spec.entity, spec.phase);
            let mut desc: sys::ecs_system_desc_t = unsafe { std::mem::zeroed() };
            desc.entity = spec.entity;
            desc.query.expr = cexpr.as_ptr();
            desc.query.order_by = reg.sid_type;
            desc.query.order_by_callback = Some(cmp_sid);
            desc.callback = Some(run);
            desc.callback_ctx = ctx;
            desc.callback_ctx_free = Some(free_ctx);
            desc.immediate = spec.immediate;
            unsafe { sys::ecs_system_init(w, &desc) }
        };
        if created == 0 {
            return Err(format!("{label}: Flecs rejected it (see the log above)"));
        }
        Ok(())
    })();
    unsafe { sys::ecs_set_scope(w, old_scope) };
    result
}

unsafe extern "C-unwind" fn run(it: *mut sys::ecs_iter_t) {
    let it = unsafe { &*it };
    let ctx = unsafe { &*(it.callback_ctx as *const Ctx) };
    let Some(reg) = ctx.reg.upgrade() else { return };
    if !reg.alive.get() || reg.faulted() {
        return;
    }
    if ctx.observer {
        let first = if it.count > 0 {
            unsafe { *it.entities }
        } else {
            0
        };
        let key = (it.event_cur, first);
        if ctx.last_event.get() == key {
            return;
        }
        ctx.last_event.set(key);
    }
    let rows = if ctx.observer {
        (it.count as usize).max(1)
    } else {
        it.count as usize
    };
    Python::attach(|py| {
        if let Err(e) = run_chunk(py, &reg, ctx, it, rows) {
            reg.set_fault(format!("{}: {}", ctx.label, sage_script::describe(py, &e)));
        }
    });
}

/// What a field parameter looks like for one table (constant per chunk).
enum Prepared<'py> {
    Entity,
    Event,
    Absent,
    /// A reflected struct: views are reused across rows when the callback
    /// kept no reference to the previous one.
    View {
        class: Bound<'py, PyAny>,
        base: *mut u8,
        stride: usize,
        shared: bool,
        readonly: bool,
        size: usize,
        cached: Option<Bound<'py, PyAny>>,
    },
    /// Opaque (Python object) or anything else: built per row.
    Other {
        id: u64,
        base: *mut u8,
        stride: usize,
        shared: bool,
        readonly: bool,
    },
}

fn prepare<'py>(
    py: Python<'py>,
    reg: &Rc<Registry>,
    ctx: &Ctx,
    it: &sys::ecs_iter_t,
) -> PyResult<Vec<Prepared<'py>>> {
    let mut out = Vec::with_capacity(ctx.params.len());
    for p in &ctx.params {
        out.push(match *p {
            Param::Entity => Prepared::Entity,
            Param::Event => Prepared::Event,
            Param::Field { index, readonly } => unsafe {
                if !sys::ecs_field_is_set(it, index) {
                    Prepared::Absent
                } else {
                    let id = sys::ecs_field_id(it, index);
                    let base =
                        sys::ecs_field_w_size(it, sys::ecs_field_size(it, index), index) as *mut u8;
                    // The stride is the matched component's size, which can
                    // exceed the term's when the match is a derived component.
                    let stride = ffi::type_size(reg.world, id);
                    let shared = !sys::ecs_field_is_self(it, index);
                    let readonly = readonly || shared;
                    let is_struct = matches!(
                        reg.types.borrow().get(&id).map(|t| &t.layout),
                        Some(Layout::Struct(_))
                    );
                    if is_struct {
                        Prepared::View {
                            class: class_for(py, reg, id)?,
                            base,
                            stride,
                            shared,
                            readonly,
                            size: stride,
                            cached: None,
                        }
                    } else {
                        Prepared::Other {
                            id,
                            base,
                            stride,
                            shared,
                            readonly,
                        }
                    }
                }
            },
        });
    }
    Ok(out)
}

fn run_chunk(
    py: Python<'_>,
    reg: &Rc<Registry>,
    ctx: &Ctx,
    it: &sys::ecs_iter_t,
    rows: usize,
) -> PyResult<()> {
    let func = ctx.func.bind(py);
    let mut prepared = prepare(py, reg, ctx, it)?;
    let mut args = Vec::with_capacity(prepared.len());
    for row in 0..rows {
        reg.push_scope();
        let result = (|| -> PyResult<()> {
            args.clear();
            for p in prepared.iter_mut() {
                let arg = match p {
                    Prepared::Entity => {
                        let e = if it.count > 0 {
                            unsafe { *it.entities.add(row) }
                        } else {
                            0
                        };
                        Py::new(py, Entity::new(reg.id, e))?
                            .into_bound(py)
                            .into_any()
                    }
                    Prepared::Event => {
                        if it.param.is_null() {
                            py.None().into_bound(py)
                        } else {
                            object_at(py, reg, ctx.event_type, it.param as *mut u8, true)?
                        }
                    }
                    Prepared::Absent => py.None().into_bound(py),
                    Prepared::View {
                        class,
                        base,
                        stride,
                        shared,
                        readonly,
                        size,
                        cached,
                    } => {
                        let ptr = if *shared {
                            *base
                        } else {
                            unsafe { base.add(row * *stride) }
                        };
                        match cached {
                            // Only our reference is left: rebind it to this row.
                            Some(obj) if unsafe { pyo3::ffi::Py_REFCNT(obj.as_ptr()) } == 1 => {
                                value::rebind(obj, reg, ptr, *readonly)?;
                                obj.clone()
                            }
                            _ => {
                                let obj =
                                    value::make_object(py, reg, class, ptr, *size, *readonly)?;
                                *cached = Some(obj.clone());
                                obj
                            }
                        }
                    }
                    Prepared::Other {
                        id,
                        base,
                        stride,
                        shared,
                        readonly,
                    } => {
                        let ptr = if *shared {
                            *base
                        } else {
                            unsafe { base.add(row * *stride) }
                        };
                        object_at(py, reg, *id, ptr, *readonly)?
                    }
                };
                args.push(arg);
            }
            let tuple = PyTuple::new(py, args.drain(..))?;
            func.call1(tuple)?;
            Ok(())
        })();
        reg.pop_scope();
        result?;
    }
    Ok(())
}
