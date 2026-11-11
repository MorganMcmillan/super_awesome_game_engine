//! Python bindings to Flecs: write components, systems and observers in
//! Python (design doc 3.1, 5.4).
//!
//! ```text
//! host (Rust)                      script (Python)
//! Ecs::new(&world, seed, "pong")   @component class Velocity: ...
//! ecs.add_native_module(..)        play = world.module("play")
//! ecs.load("pong", source)  ---->  @play.system(phase=SimMove)
//! world.progress()                 def move(pos: Position, vel: Velocity): ...
//! ecs.hash_into(&mut h)
//! ```
//!
//! - Components are reflected Flecs structs by default; Python objects are
//!   values or views onto the stored bytes (`value.rs`). Opaque components
//!   store a Python object instead.
//! - Systems run once per matched entity, in StableId order
//!   (`callbacks.rs`).
//! - Hashing, snapshots and restore are generic over everything the binding
//!   knows (`state.rs`).
//! - Native components under `sage.` are importable by path
//!   (`from sage.collision import Position`), and `Ecs::type_stubs` writes
//!   editor stubs for them (`stubs.rs`).
//!
//! Like `sage_script`, this runs trusted code only: there is no sandbox.

mod api;
mod callbacks;
mod ffi;
mod registry;
mod scene;
mod state;
mod stubs;
mod value;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::rc::Rc;
use std::sync::OnceLock;

use flecs_ecs::prelude::*;
use flecs_ecs::sys;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use sage_core::{Rng, StateHasher};
use sage_script::{Script, ScriptError, describe};

pub use registry::Kind;
pub use sage_scene::{Policy, StringTable};
pub use scene::LoadedGame;
pub use state::{EntitySnapshot, Snapshot, StoredValue};

use api::NativeWorld;
use registry::Registry;

/// `sage.ecs`, from the shared package directory (`python/sage/`).
const ECS_SOURCE: &str = include_str!("../../../python/sage/ecs.py");

/// Names re-exported from `sage.ecs` into the `sage` package.
const EXPORTS: &[&str] = &[
    "component",
    "Component",
    "Entity",
    "World",
    "FlecsModule",
    "Prefab",
    "TICK_DT",
    "f32",
    "f64",
    "i32",
    "u32",
    "i64",
    "u64",
    "u8",
    "OnLoad",
    "PostLoad",
    "PreUpdate",
    "OnUpdate",
    "OnValidate",
    "PostUpdate",
    "PreStore",
    "OnStore",
    "OnAdd",
    "OnSet",
    "OnRemove",
];

/// The engine's StableId component, `sage.StableId`. Every entity the
/// binding creates has one; Python systems only see entities that do.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
#[flecs(meta)]
pub struct StableIdC {
    pub value: u64,
}

static INSTALLED: OnceLock<Result<(), ScriptError>> = OnceLock::new();

/// Adds `sage.ecs` (and `sage._ecs`) to the scripting package, once.
pub fn install() -> Result<(), ScriptError> {
    sage_script::init()?;
    INSTALLED
        .get_or_init(|| {
            Python::attach(|py| {
                let fail = |e: PyErr| describe(py, &e);
                let sage = sage_script::package(py)?;
                let native = PyModule::new(py, "sage._ecs").map_err(fail)?;
                native.add_class::<value::Value>().map_err(fail)?;
                native.add_class::<value::Field>().map_err(fail)?;
                native.add_class::<api::Entity>().map_err(fail)?;
                native.add_class::<api::WorldRng>().map_err(fail)?;
                native.add_class::<NativeWorld>().map_err(fail)?;
                let modules = py
                    .import("sys")
                    .and_then(|s| s.getattr("modules"))
                    .map_err(fail)?;
                modules.set_item("sage._ecs", &native).map_err(fail)?;
                sage.setattr("_ecs", &native).map_err(fail)?;
                let code = CString::new(ECS_SOURCE).unwrap();
                let ecs =
                    PyModule::from_code(py, &code, c"<sage.ecs>", c"sage.ecs").map_err(fail)?;
                sage.setattr("ecs", &ecs).map_err(fail)?;
                for name in EXPORTS {
                    sage.setattr(*name, ecs.getattr(*name).map_err(fail)?)
                        .map_err(fail)?;
                }
                Ok(())
            })
        })
        .clone()
}

/// The binding for one world. Create it before loading scripts; keep it
/// alive as long as the world runs Python systems.
pub struct Ecs {
    reg: Rc<Registry>,
    /// Components added with `register_native`, in registration order.
    natives: std::cell::RefCell<Vec<u64>>,
}

impl Ecs {
    /// Binds `world`. Scripts' components and systems go into a flecs
    /// module named `module_name`; `seed` seeds the world RNG.
    pub fn new(world: &World, seed: u64, module_name: &str) -> Result<Ecs, ScriptError> {
        install()?;
        let sid = world.component_named::<StableIdC>("sage::StableId");
        let w = world.ptr_mut();
        // A prefab's StableId must not be copied to its instances, or every
        // instance would share it (verified: tests/flecs_assumptions.rs).
        ffi::add(
            w,
            *sid.id(),
            ffi::pair(unsafe { sys::EcsOnInstantiate }, unsafe {
                sys::EcsDontInherit
            }),
        );
        let reg = Registry::new(w, *sid.id(), seed);
        reg.register_reflected(reg.sid_type).map_err(ScriptError)?;
        let module = ffi::named_child(w, 0, module_name);
        ffi::add(w, module, unsafe { sys::EcsModule });
        reg.script_module.set(module);
        reg.modules.borrow_mut().push(module);
        Python::attach(|py| {
            let native = Py::new(py, NativeWorld { reg: reg.id }).map_err(|e| describe(py, &e))?;
            let wrapper = py
                .import("sage.ecs")
                .and_then(|m| m.getattr("World"))
                .and_then(|c| c.call1((native,)))
                .map_err(|e| describe(py, &e))?;
            *reg.world_obj.borrow_mut() = Some(wrapper.unbind());
            Ok::<_, ScriptError>(())
        })?;
        Ok(Ecs {
            reg,
            natives: Default::default(),
        })
    }

    /// Makes a native reflected component (`#[flecs(meta)]`) known: it is
    /// hashed and snapshotted, and Python can use it by path. A component
    /// whose path is under `sage.` can also be imported by that path while
    /// a script loads (`sage::collision::Position` is
    /// `from sage.collision import Position`).
    pub fn register_native(&self, id: impl Into<u64>) -> Result<(), String> {
        let id = id.into();
        self.reg.register_reflected(id)?;
        let path = self.reg.types.borrow()[&id].path.clone();
        Python::attach(|py| {
            py.import("sage.ecs")
                .and_then(|m| m.call_method1("_expose_native", (&path,)))
                .map(|_| ())
                .map_err(|e| describe(py, &e).0)
        })?;
        let mut natives = self.natives.borrow_mut();
        if !natives.contains(&id) {
            natives.push(id);
        }
        Ok(())
    }

    /// Editor stubs for the importable native components registered so far:
    /// (file path relative to the `sage` package directory, contents),
    /// sorted by path. Components directly under `sage` go to `_native.pyi`,
    /// which the package's `__init__.py` star-imports for editors.
    pub fn type_stubs(&self) -> Vec<(String, String)> {
        stubs::generate(&self.reg, &self.natives.borrow())
    }

    /// A native flecs module scripts can instantiate with
    /// `world.import_native(name, phase)` or `module.import_native(..)`.
    /// The closure gets the raw world, the phase and the parent module
    /// entity, and returns the new module entity.
    pub fn add_native_module(
        &self,
        name: &str,
        f: impl Fn(*mut sys::ecs_world_t, u64, u64) -> Result<u64, String> + 'static,
    ) {
        self.reg
            .native_modules
            .borrow_mut()
            .insert(name.to_string(), Box::new(f));
    }

    /// Runs a script with `world` in its globals. Components, flecs modules,
    /// systems and observers it declares are registered as it runs.
    pub fn load(&self, name: &str, source: &str) -> Result<Script, ScriptError> {
        let world_obj = Python::attach(|py| {
            self.reg
                .world_obj
                .borrow()
                .as_ref()
                .map(|w| w.clone_ref(py))
                .expect("set in new")
        });
        Python::attach(|py| -> PyResult<()> {
            py.import("sage.ecs")?
                .call_method1("_push_loading", (world_obj.clone_ref(py),))?;
            Ok(())
        })
        .map_err(|e| Python::attach(|py| describe(py, &e)))?;
        self.reg.push_scope();
        let result = Script::load_with(name, source, vec![("world".to_string(), world_obj)]);
        self.reg.pop_scope();
        self.reg.invalidate_views();
        Python::attach(|py| {
            let _ = py
                .import("sage.ecs")
                .and_then(|m| m.call_method0("_pop_loading"));
        });
        let script = result?;
        if let Some(f) = self.fault() {
            return Err(ScriptError(f));
        }
        Ok(script)
    }

    /// The first error raised by a Python system or observer, if any. After
    /// a fault every Python callback is skipped until `clear_fault`.
    pub fn fault(&self) -> Option<String> {
        self.reg.fault.borrow().clone()
    }

    pub fn clear_fault(&self) {
        *self.reg.fault.borrow_mut() = None;
    }

    pub fn sid_type(&self) -> u64 {
        self.reg.sid_type
    }

    /// The flecs module scripts are loaded into.
    pub fn script_module(&self) -> u64 {
        self.reg.script_module.get()
    }

    pub fn entity_of_sid(&self, sid: u64) -> Option<u64> {
        self.reg.entity_of_sid(sid).or_else(|| {
            state::entities(&self.reg);
            self.reg.entity_of_sid(sid)
        })
    }

    pub fn sid_of(&self, entity: u64) -> Option<u64> {
        self.reg.sid_of(entity)
    }

    /// Gives a natively created entity a StableId so Python systems see it.
    pub fn assign_sid(&self, entity: u64, sid: Option<u64>) -> Result<u64, String> {
        self.reg.assign_sid(entity, sid)
    }

    pub fn rng_state(&self) -> Rng {
        self.reg.rng.borrow().clone()
    }

    /// Feeds everything the binding knows into `h`, in canonical order.
    pub fn hash_into(&self, h: &mut StateHasher) {
        state::hash_into(&self.reg, h);
    }

    pub fn snapshot(&self) -> Snapshot {
        state::snapshot(&self.reg)
    }

    /// Restores a snapshot from a world that loaded the same scripts.
    pub fn restore(&self, s: &Snapshot) -> Result<(), String> {
        state::restore(&self.reg, s)
    }

    /// The schema (version, field names and kinds) of every reflected
    /// component, as scenes and save games record it.
    pub fn schemas(&self) -> sage_scene::Schemas {
        scene::schemas(&self.reg)
    }

    /// Adds schema migrations in the `sage-migrations v1` text form (kits
    /// ship them with their components). Scripts can also call
    /// `world.migrations(text)`.
    pub fn add_migrations(&self, text: &str) -> Result<(), String> {
        self.reg
            .migrations
            .borrow_mut()
            .add_text(text)
            .map_err(|e| e.to_string())
    }

    /// The world's string table, shared with native code that draws or
    /// edits text (components store only the ids).
    pub fn strings(&self) -> Rc<RefCell<StringTable>> {
        self.reg.strings.clone()
    }

    /// Interns `text` and returns its id (0 for the empty string).
    pub fn intern(&self, text: &str) -> u64 {
        self.reg.strings.borrow_mut().intern(text)
    }

    /// The entities and singletons as a `sage-scene v1` document.
    pub fn save_scene(&self) -> String {
        scene::save_scene(&self.reg)
    }

    /// Replaces the world's entities and singletons with a scene's, after
    /// migrating it to the current schema. Returns what was migrated and
    /// any warnings (non-empty only under [`Policy::Lenient`]). Call it
    /// between ticks.
    pub fn load_scene(&self, text: &str, policy: Policy) -> Result<Vec<String>, String> {
        scene::load_scene(&self.reg, text, policy)
    }

    /// A `sage-save v1` document: the scene plus the RNG, the StableId
    /// counter, flecs module switches, and the host's `extra` entries (state
    /// the binding does not know, such as a contact table).
    pub fn save_game(&self, extra: BTreeMap<String, String>) -> String {
        scene::save_game(&self.reg, extra)
    }

    /// Restores a save game into a world that loaded the same scripts.
    /// Returns the host extras that were saved.
    pub fn load_game(&self, text: &str, policy: Policy) -> Result<LoadedGame, String> {
        scene::load_game(&self.reg, text, policy)
    }

    /// True if some simulation entity holds an opaque component, which
    /// hashing and snapshots skip.
    pub fn has_opaque_state(&self) -> bool {
        state::has_opaque_state(&self.reg)
    }

    /// The entity at a dotted path (`pong.Ball`), or 0.
    pub fn lookup_path(&self, path: &str) -> u64 {
        ffi::lookup_path(self.reg.world, path)
    }

    /// Reads one field of a reflected component by name, through reflection
    /// (for hosts reading components defined in Python). Entity fields read
    /// as the target's StableId.
    pub fn field(&self, entity: u64, component: u64, field: &str) -> Option<f64> {
        let w = self.reg.world;
        if !ffi::alive(w, entity) || !ffi::has(w, entity, component) {
            return None;
        }
        let f = match &self.reg.types.borrow().get(&component)?.layout {
            registry::Layout::Struct(fields) => fields.iter().find(|f| f.name == field)?.clone(),
            _ => return None,
        };
        let base = unsafe { sys::ecs_get_id(w, entity, component) } as *const u8;
        let p = unsafe { base.add(f.offset) };
        Some(unsafe {
            match f.kind {
                Kind::Bool | Kind::U8 => *p as f64,
                Kind::I32 => *(p as *const i32) as f64,
                Kind::U32 => *(p as *const u32) as f64,
                Kind::I64 => *(p as *const i64) as f64,
                Kind::U64 => *(p as *const u64) as f64,
                Kind::F32 => *(p as *const f32) as f64,
                Kind::F64 => *(p as *const f64),
                Kind::Entity => self.reg.sid_of(*(p as *const u64)).unwrap_or(0) as f64,
            }
        })
    }

    pub fn has(&self, entity: u64, id: u64) -> bool {
        ffi::alive(self.reg.world, entity) && ffi::has(self.reg.world, entity, id)
    }

    /// Paths of the flecs modules created through the binding and whether
    /// each is enabled.
    pub fn modules(&self) -> Vec<(String, bool)> {
        let w = self.reg.world;
        self.reg
            .modules
            .borrow()
            .iter()
            .map(|e| (ffi::path(w, *e), ffi::enabled(w, *e)))
            .collect()
    }
}

impl Drop for Ecs {
    fn drop(&mut self) {
        // Python objects may outlive the world; make them fail cleanly.
        self.reg.alive.set(false);
        self.reg.invalidate_views();
        Python::attach(|_py| {
            self.reg.world_obj.borrow_mut().take();
            for t in self.reg.types.borrow_mut().values_mut() {
                t.class.take();
            }
        });
        registry::unregister(self.reg.id);
    }
}
