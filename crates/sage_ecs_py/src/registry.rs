//! Per-world binding state, shared by the host, the Python objects and the
//! system callbacks.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use flecs_ecs::sys;
use pyo3::prelude::*;
use sage_core::Rng;

use crate::ffi::{self, RawWorld};

/// The primitive field types a reflected component can have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bool,
    U8,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    /// A Flecs entity id. Hashed and snapshotted as the target's StableId.
    Entity,
}

impl Kind {
    pub fn from_name(s: &str) -> Option<Kind> {
        Some(match s {
            "bool" => Kind::Bool,
            "u8" => Kind::U8,
            "i32" => Kind::I32,
            "u32" => Kind::U32,
            "i64" => Kind::I64,
            "u64" => Kind::U64,
            "f32" => Kind::F32,
            "f64" => Kind::F64,
            "entity" => Kind::Entity,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Bool => "bool",
            Kind::U8 => "u8",
            Kind::I32 => "i32",
            Kind::U32 => "u32",
            Kind::I64 => "i64",
            Kind::U64 => "u64",
            Kind::F32 => "f32",
            Kind::F64 => "f64",
            Kind::Entity => "entity",
        }
    }

    pub fn type_id(self) -> u64 {
        unsafe {
            match self {
                Kind::Bool => sys::FLECS_IDecs_bool_tID_,
                Kind::U8 => sys::FLECS_IDecs_u8_tID_,
                Kind::I32 => sys::FLECS_IDecs_i32_tID_,
                Kind::U32 => sys::FLECS_IDecs_u32_tID_,
                Kind::I64 => sys::FLECS_IDecs_i64_tID_,
                Kind::U64 => sys::FLECS_IDecs_u64_tID_,
                Kind::F32 => sys::FLECS_IDecs_f32_tID_,
                Kind::F64 => sys::FLECS_IDecs_f64_tID_,
                Kind::Entity => sys::FLECS_IDecs_entity_tID_,
            }
        }
    }

    pub fn from_type_id(id: u64) -> Option<Kind> {
        [
            Kind::Bool,
            Kind::U8,
            Kind::I32,
            Kind::U32,
            Kind::I64,
            Kind::U64,
            Kind::F32,
            Kind::F64,
            Kind::Entity,
        ]
        .into_iter()
        .find(|k| k.type_id() == id)
    }

    pub fn size(self) -> usize {
        match self {
            Kind::Bool | Kind::U8 => 1,
            Kind::I32 | Kind::U32 | Kind::F32 => 4,
            Kind::I64 | Kind::U64 | Kind::F64 | Kind::Entity => 8,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FieldInfo {
    pub name: String,
    pub offset: usize,
    pub kind: Kind,
}

#[derive(Clone, Debug)]
pub enum Layout {
    /// A reflected struct; the stored bytes are the value.
    Struct(Vec<FieldInfo>),
    /// No data.
    Tag,
    /// A pointer to a Python object (`@component(opaque=True)`). Not hashed
    /// or snapshotted (design doc: [Open]).
    Opaque,
}

pub struct TypeInfo {
    pub path: String,
    pub size: usize,
    pub layout: Layout,
    /// The Python class for this component, once one exists.
    pub class: Option<Py<PyAny>>,
}

/// Native code a Python script can import as a flecs module, e.g. collision.
/// Receives the world, the phase to run in and the parent flecs module, and
/// returns the new flecs module entity.
pub type NativeModule = Box<dyn Fn(RawWorld, u64, u64) -> Result<u64, String>>;

/// Python objects refer to their world by id, never by `Rc`: Python may free
/// them on any thread (script classes and globals are cyclic garbage), and
/// an id is safe to drop anywhere. Lookups only succeed on the thread that
/// owns the world.
pub type RegId = u64;

static NEXT_REG_ID: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static REGISTRIES: RefCell<BTreeMap<RegId, Rc<Registry>>> = const { RefCell::new(BTreeMap::new()) };
}

/// The live registry with this id, if it belongs to this thread.
pub fn lookup(id: RegId) -> Option<Rc<Registry>> {
    REGISTRIES
        .with(|r| r.borrow().get(&id).cloned())
        .filter(|r| r.alive.get())
}

pub fn unregister(id: RegId) {
    let reg = REGISTRIES.with(|r| r.borrow_mut().remove(&id));
    drop(reg);
}

/// Whether a view (a Python object pointing into Flecs storage) may still be
/// used. A view is valid while the callback that made it is running and no
/// structural change has happened since, because structural changes move
/// component data.
#[derive(Clone, Copy, Debug)]
pub struct Validity {
    pub depth: usize,
    pub serial: u64,
    pub epoch: u64,
}

pub struct Registry {
    pub id: RegId,
    pub world: RawWorld,
    pub alive: Cell<bool>,
    scopes: RefCell<Vec<u64>>,
    next_serial: Cell<u64>,
    epoch: Cell<u64>,
    pub types: RefCell<BTreeMap<u64, TypeInfo>>,
    /// Flecs modules created through the binding; their enabled state is
    /// simulation state.
    pub modules: RefCell<Vec<u64>>,
    pub fault: RefCell<Option<String>>,
    /// `sage.StableId`.
    pub sid_type: u64,
    pub next_sid: Cell<u64>,
    /// StableId -> entity. Ordered, so iterating it is canonical.
    pub sids: RefCell<BTreeMap<u64, u64>>,
    pub rng: RefCell<Rng>,
    /// The flecs module the script's components and systems live in.
    pub script_module: Cell<u64>,
    pub native_modules: RefCell<BTreeMap<String, NativeModule>>,
    /// The Python `sage.ecs.World` wrapper for this world.
    pub world_obj: RefCell<Option<Py<PyAny>>>,
}

impl Registry {
    pub fn new(world: RawWorld, sid_type: u64, seed: u64) -> Rc<Registry> {
        let reg = Rc::new(Registry {
            id: NEXT_REG_ID.fetch_add(1, Ordering::Relaxed),
            world,
            alive: Cell::new(true),
            scopes: RefCell::new(vec![]),
            next_serial: Cell::new(1),
            epoch: Cell::new(0),
            types: RefCell::new(BTreeMap::new()),
            modules: RefCell::new(vec![]),
            fault: RefCell::new(None),
            sid_type,
            next_sid: Cell::new(1),
            sids: RefCell::new(BTreeMap::new()),
            rng: RefCell::new(Rng::from_seed(seed)),
            script_module: Cell::new(0),
            native_modules: RefCell::new(BTreeMap::new()),
            world_obj: RefCell::new(None),
        });
        REGISTRIES.with(|r| r.borrow_mut().insert(reg.id, reg.clone()));
        reg
    }

    /// Enters a callback scope. Views made inside it die when it is left.
    pub fn push_scope(&self) {
        let s = self.next_serial.get();
        self.next_serial.set(s + 1);
        self.scopes.borrow_mut().push(s);
    }

    pub fn pop_scope(&self) {
        self.scopes.borrow_mut().pop();
    }

    /// The validity stamp for a view made now, if views can be made now.
    pub fn stamp(&self) -> Option<Validity> {
        let scopes = self.scopes.borrow();
        let serial = *scopes.last()?;
        Some(Validity {
            depth: scopes.len() - 1,
            serial,
            epoch: self.epoch.get(),
        })
    }

    pub fn is_valid(&self, v: &Validity) -> bool {
        self.alive.get()
            && v.epoch == self.epoch.get()
            && self.scopes.borrow().get(v.depth) == Some(&v.serial)
    }

    /// Call after any operation that may move component data. In deferred
    /// mode (inside a system) operations are queued, so nothing moves.
    pub fn structural_change(&self) {
        if !ffi::deferred(self.world) {
            self.invalidate_views();
        }
    }

    pub fn invalidate_views(&self) {
        self.epoch.set(self.epoch.get() + 1);
    }

    pub fn class_of(&self, id: u64) -> Option<Py<PyAny>> {
        Python::attach(|py| {
            self.types
                .borrow()
                .get(&id)
                .and_then(|t| t.class.as_ref().map(|c| c.clone_ref(py)))
        })
    }

    pub fn sid_of(&self, e: u64) -> Option<u64> {
        if !ffi::alive(self.world, e) || !ffi::has(self.world, e, self.sid_type) {
            return None;
        }
        let p = unsafe { sys::ecs_get_id(self.world, e, self.sid_type) } as *const u64;
        Some(unsafe { *p })
    }

    pub fn entity_of_sid(&self, sid: u64) -> Option<u64> {
        let e = *self.sids.borrow().get(&sid)?;
        ffi::alive(self.world, e).then_some(e)
    }

    /// Gives `e` a StableId (the next free one unless `sid` is given).
    pub fn assign_sid(&self, e: u64, sid: Option<u64>) -> Result<u64, String> {
        let sid = match sid {
            Some(s) => s,
            None => {
                let mut s = self.next_sid.get();
                while self.sids.borrow().contains_key(&s) {
                    s += 1;
                }
                s
            }
        };
        if let Some(other) = self.entity_of_sid(sid)
            && other != e
        {
            return Err(format!("StableId {sid} is already used by another entity"));
        }
        if sid >= self.next_sid.get() {
            self.next_sid.set(sid + 1);
        }
        unsafe {
            sys::ecs_set_id(
                self.world,
                e,
                self.sid_type,
                8,
                &sid as *const u64 as *const std::ffi::c_void,
            )
        };
        self.sids.borrow_mut().insert(sid, e);
        Ok(sid)
    }

    pub fn set_fault(&self, msg: String) {
        let mut f = self.fault.borrow_mut();
        if f.is_none() {
            *f = Some(msg);
        }
    }

    pub fn faulted(&self) -> bool {
        self.fault.borrow().is_some()
    }

    /// Registers a reflected struct type that already exists in Flecs
    /// (native or created by the binding) by reading its members.
    pub fn register_reflected(&self, id: u64) -> Result<(), String> {
        if self.types.borrow().contains_key(&id) {
            return Ok(());
        }
        let w = self.world;
        let path = ffi::path(w, id);
        let size = ffi::type_size(w, id);
        let st =
            unsafe { sys::ecs_get_id(w, id, sys::FLECS_IDEcsStructID_) } as *const sys::EcsStruct;
        let layout = if st.is_null() {
            if size == 0 {
                Layout::Tag
            } else {
                return Err(format!("{path} has no reflection data"));
            }
        } else {
            let v = unsafe { &(*st).members };
            let members = unsafe {
                std::slice::from_raw_parts(v.array as *const sys::ecs_member_t, v.count as usize)
            };
            let mut fields = vec![];
            for m in members {
                let name = unsafe { std::ffi::CStr::from_ptr(m.name) }
                    .to_string_lossy()
                    .into_owned();
                let kind = Kind::from_type_id(m.type_).ok_or_else(|| {
                    format!(
                        "{path}.{name}: field type {} is not supported by the Python binding",
                        ffi::path(w, m.type_)
                    )
                })?;
                if m.count > 1 {
                    return Err(format!("{path}.{name}: arrays are not supported yet"));
                }
                fields.push(FieldInfo {
                    name,
                    offset: m.offset as usize,
                    kind,
                });
            }
            Layout::Struct(fields)
        };
        self.types.borrow_mut().insert(
            id,
            TypeInfo {
                path,
                size,
                layout,
                class: None,
            },
        );
        Ok(())
    }
}
