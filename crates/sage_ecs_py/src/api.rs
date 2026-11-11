//! The native half of the Python API (`sage._ecs`). The Python half
//! (`sage.ecs`, `python/ecs.py`) wraps it with decorators and signature
//! analysis.

use std::ffi::c_void;
use std::rc::Rc;

use flecs_ecs::sys;
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple, PyType};

use crate::ffi::{self, RawWorld};
use crate::registry::{FieldInfo, Kind, Layout, RegId, Registry, TypeInfo};
use crate::value::{self, Field};

/// (name, kind, offset) for each field, as Python sees them.
type FieldTuples = Vec<(String, String, usize)>;

pub(crate) fn upgrade(reg: &RegId) -> PyResult<Rc<Registry>> {
    crate::registry::lookup(*reg)
        .filter(|r| r.alive.get())
        .ok_or_else(|| PyRuntimeError::new_err("the world this object belongs to is gone"))
}

/// The component (or entity) id of `obj`: an Entity, an int, or a class
/// with `__sage_id__`.
pub(crate) fn id_of(obj: &Bound<'_, PyAny>) -> PyResult<u64> {
    if let Ok(e) = obj.cast::<Entity>() {
        return Ok(e.borrow().id);
    }
    if let Ok(i) = obj.extract::<u64>() {
        return Ok(i);
    }
    if let Ok(id) = obj.getattr(pyo3::intern!(obj.py(), "__sage_id__")) {
        return id.extract();
    }
    if let Ok(e) = obj.getattr(pyo3::intern!(obj.py(), "entity"))
        && let Ok(e) = e.cast::<Entity>()
    {
        return Ok(e.borrow().id);
    }
    Err(PyTypeError::new_err(format!(
        "expected a component class, an Entity or a flecs module, got {}",
        obj.repr()?
    )))
}

/// What one argument of `add`, `remove` or `has` names.
enum Item<'py> {
    /// A tag, component class, entity or flecs module. The argument is kept
    /// so `add` can build a default value from a component class.
    Id(u64, Bound<'py, PyAny>),
    /// A `(relationship, target)` tuple.
    Pair(u64),
    /// A component instance. `add` stores its value; `remove` and `has` use
    /// only its type.
    Value(u64, Bound<'py, PyAny>),
}

impl Item<'_> {
    fn id(&self) -> u64 {
        match self {
            Item::Id(id, _) | Item::Pair(id) | Item::Value(id, _) => *id,
        }
    }
}

fn item_of<'py>(obj: &Bound<'py, PyAny>) -> PyResult<Item<'py>> {
    if let Ok(t) = obj.cast::<PyTuple>() {
        if t.len() != 2 {
            return Err(PyTypeError::new_err(format!(
                "a pair is a (relationship, target) tuple, got {} items",
                t.len()
            )));
        }
        let rel = id_of(&t.get_item(0)?)?;
        let target = id_of(&t.get_item(1)?)?;
        return Ok(Item::Pair(ffi::pair(rel, target)));
    }
    // An instance of a component class carries `__sage_id__` through its
    // class, so it must be recognized before `id_of` would reduce it to the
    // component's id and lose its value.
    let plain = obj.is_instance_of::<PyType>()
        || obj.cast::<Entity>().is_ok()
        || obj.extract::<u64>().is_ok();
    if !plain {
        let ty = obj.get_type().into_any();
        if ty.hasattr(pyo3::intern!(obj.py(), "__sage_id__"))? {
            return Ok(Item::Value(id_of(&ty)?, obj.clone()));
        }
    }
    Ok(Item::Id(id_of(obj)?, obj.clone()))
}

/// Every argument, classified. Nothing has been changed yet if this fails.
fn items_of<'py>(method: &str, args: &Bound<'py, PyTuple>) -> PyResult<Vec<Item<'py>>> {
    if args.is_empty() {
        return Err(PyTypeError::new_err(format!(
            "{method}() needs at least one tag, component or pair"
        )));
    }
    args.iter().map(|a| item_of(&a)).collect()
}

fn type_layout(reg: &Registry, id: u64) -> PyResult<(usize, Layout)> {
    let types = reg.types.borrow();
    let t = types.get(&id).ok_or_else(|| {
        PyKeyError::new_err(format!(
            "{} is not a component known to the Python binding",
            ffi::path(reg.world, id)
        ))
    })?;
    Ok((t.size, t.layout.clone()))
}

/// Python object for the component `id` stored at `ptr`.
pub(crate) fn object_at<'py>(
    py: Python<'py>,
    reg: &Rc<Registry>,
    id: u64,
    ptr: *mut u8,
    readonly: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let (size, layout) = type_layout(reg, id)?;
    match layout {
        Layout::Opaque => {
            let obj = unsafe { *(ptr as *mut *mut pyo3::ffi::PyObject) };
            if obj.is_null() {
                Ok(py.None().into_bound(py))
            } else {
                Ok(unsafe { Bound::from_borrowed_ptr(py, obj) })
            }
        }
        _ => {
            let class = class_for(py, reg, id)?;
            value::make_object(py, reg, &class, ptr, size, readonly)
        }
    }
}

/// The Python class for component `id`, creating one for a native reflected
/// component on first use.
pub(crate) fn class_for<'py>(
    py: Python<'py>,
    reg: &Rc<Registry>,
    id: u64,
) -> PyResult<Bound<'py, PyAny>> {
    if let Some(c) = reg.class_of(id) {
        return Ok(c.into_bound(py));
    }
    reg.register_reflected(id).map_err(PyTypeError::new_err)?;
    let world = reg
        .world_obj
        .borrow()
        .as_ref()
        .map(|w| w.clone_ref(py))
        .ok_or_else(|| PyRuntimeError::new_err("world is not initialized"))?;
    let class = py
        .import("sage.ecs")?
        .getattr("_class_for_native")?
        .call1((world, id))?;
    Ok(class)
}

/// An entity handle. Cheap to copy; does not keep the entity alive.
#[pyclass(frozen, module = "sage")]
pub struct Entity {
    pub reg: RegId,
    pub id: u64,
}

impl Entity {
    pub fn new(reg: RegId, id: u64) -> Self {
        Entity { reg, id }
    }

    /// Adds or removes `Disabled`. Only this entity moves tables, so views
    /// elsewhere stay valid unless it carries a component Python can view.
    fn set_enabled(&self, on: bool) -> PyResult<()> {
        let (reg, w) = self.live()?;
        if ffi::enabled(w, self.id) == on {
            return Ok(());
        }
        unsafe { sys::ecs_enable(w, self.id, on) };
        let viewable = {
            let types = reg.types.borrow();
            ffi::entity_ids(w, self.id)
                .iter()
                .any(|id| types.get(id).is_some_and(|t| t.size > 0))
        };
        if viewable {
            reg.structural_change();
        }
        Ok(())
    }

    fn live(&self) -> PyResult<(Rc<Registry>, RawWorld)> {
        let reg = upgrade(&self.reg)?;
        if !ffi::alive(reg.world, self.id) {
            return Err(PyRuntimeError::new_err(format!(
                "entity {} is not alive",
                self.id & 0xFFFF_FFFF
            )));
        }
        let w = reg.world;
        Ok((reg, w))
    }
}

pub(crate) fn set_value(reg: &Rc<Registry>, e: u64, value: &Bound<'_, PyAny>) -> PyResult<()> {
    let w = reg.world;
    let id = id_of(&value.get_type().into_any())?;
    let (size, layout) = type_layout(reg, id)?;
    let new = !ffi::has(w, e, id);
    match layout {
        Layout::Tag => ffi::add(w, e, id),
        Layout::Struct(_) => {
            let p = value::data_ptr(value)?;
            unsafe { sys::ecs_set_id(w, e, id, size, p as *const c_void) };
        }
        Layout::Opaque => {
            // The copy hook deep-copies, like any other set.
            let p = value.as_ptr();
            unsafe {
                sys::ecs_set_id(
                    w,
                    e,
                    id,
                    size,
                    &p as *const *mut pyo3::ffi::PyObject as *const c_void,
                )
            };
        }
    }
    if new {
        reg.structural_change();
    }
    Ok(())
}

pub(crate) fn get_value<'py>(
    py: Python<'py>,
    reg: &Rc<Registry>,
    e: u64,
    cls: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let w = reg.world;
    let id = id_of(cls)?;
    if !ffi::has(w, e, id) {
        return Ok(None);
    }
    let (_, layout) = type_layout(reg, id)?;
    if let Layout::Tag = layout {
        return Err(PyTypeError::new_err("a tag has no value; use has()"));
    }
    let p = unsafe { sys::ecs_get_mut_id(w, e, id) } as *mut u8;
    if p.is_null() {
        // Inherited from a prefab: read-only through the base.
        let p = unsafe { sys::ecs_get_id(w, e, id) } as *mut u8;
        return object_at(py, reg, id, p, true).map(Some);
    }
    object_at(py, reg, id, p, false).map(Some)
}

#[pymethods]
impl Entity {
    #[getter]
    fn id(&self) -> u64 {
        self.id
    }

    /// The entity's StableId, or None.
    #[getter]
    fn sid(&self) -> PyResult<Option<u64>> {
        Ok(upgrade(&self.reg)?.sid_of(self.id))
    }

    #[getter]
    fn name(&self) -> PyResult<Option<String>> {
        let reg = upgrade(&self.reg)?;
        Ok(ffi::name(reg.world, self.id))
    }

    #[getter]
    fn path(&self) -> PyResult<String> {
        let reg = upgrade(&self.reg)?;
        Ok(ffi::path(reg.world, self.id))
    }

    #[getter]
    fn is_alive(&self) -> PyResult<bool> {
        let reg = upgrade(&self.reg)?;
        Ok(ffi::alive(reg.world, self.id))
    }

    /// Sets a component value (copied in). Returns the entity for chaining.
    fn set<'py>(slf: &Bound<'py, Self>, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, Self>> {
        let (reg, _) = slf.borrow().live()?;
        set_value(&reg, slf.borrow().id, value)?;
        Ok(slf.clone())
    }

    /// Adds each argument, in order. An argument is a tag or component class
    /// (a data component starts from its defaults), a component instance
    /// (its value is stored, like `set`), or a `(relationship, target)`
    /// tuple for a pair. Every argument is checked before the first change.
    #[pyo3(signature = (*items))]
    fn add<'py>(slf: &Bound<'py, Self>, items: &Bound<'py, PyTuple>) -> PyResult<Bound<'py, Self>> {
        let (reg, w) = slf.borrow().live()?;
        let e = slf.borrow().id;
        for item in items_of("add", items)? {
            match item {
                Item::Value(_, v) => set_value(&reg, e, &v)?,
                Item::Id(id, what) => {
                    if ffi::type_size(w, id) > 0 && what.hasattr("__sage_fields__")? {
                        // A data component: add with its defaults, like set(Cls()).
                        set_value(&reg, e, &what.call0()?)?;
                    } else if !ffi::has(w, e, id) {
                        ffi::add(w, e, id);
                        reg.structural_change();
                    }
                }
                Item::Pair(id) => {
                    if !ffi::has(w, e, id) {
                        ffi::add(w, e, id);
                        reg.structural_change();
                    }
                }
            }
        }
        Ok(slf.clone())
    }

    /// Removes each argument (classes, entities, `(relationship, target)`
    /// tuples; an instance stands for its component type). Ones the entity
    /// lacks are skipped.
    #[pyo3(signature = (*items))]
    fn remove<'py>(
        slf: &Bound<'py, Self>,
        items: &Bound<'py, PyTuple>,
    ) -> PyResult<Bound<'py, Self>> {
        let (reg, w) = slf.borrow().live()?;
        let e = slf.borrow().id;
        let mut changed = false;
        for item in items_of("remove", items)? {
            let id = item.id();
            if ffi::has(w, e, id) {
                ffi::remove(w, e, id);
                changed = true;
            }
        }
        if changed {
            reg.structural_change();
        }
        Ok(slf.clone())
    }

    /// True if the entity has every argument (classes, entities,
    /// `(relationship, target)` tuples; an instance stands for its component
    /// type, its value is not compared).
    #[pyo3(signature = (*items))]
    fn has(&self, items: &Bound<'_, PyTuple>) -> PyResult<bool> {
        let (_, w) = self.live()?;
        Ok(items_of("has", items)?
            .iter()
            .all(|i| ffi::has(w, self.id, i.id())))
    }

    /// The component value: a view while a callback runs (writes go to the
    /// world), else a copy. None if the entity does not have it.
    fn get<'py>(
        &self,
        py: Python<'py>,
        cls: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let (reg, _) = self.live()?;
        get_value(py, &reg, self.id, cls)
    }

    /// The target of relationship `rel` (the first one), or None.
    #[pyo3(signature = (rel, index=0))]
    fn target(&self, rel: &Bound<'_, PyAny>, index: i32) -> PyResult<Option<Entity>> {
        let (_, w) = self.live()?;
        let t = unsafe { sys::ecs_get_target(w, self.id, id_of(rel)?, index) };
        Ok((t != 0).then(|| Entity::new(self.reg, t)))
    }

    #[getter]
    fn parent(&self) -> PyResult<Option<Entity>> {
        let (_, w) = self.live()?;
        let p = unsafe { sys::ecs_get_parent(w, self.id) };
        Ok((p != 0).then(|| Entity::new(self.reg, p)))
    }

    /// Emits `event` (a component value or class) on this entity.
    /// Observers run before this returns.
    fn emit(&self, event: &Bound<'_, PyAny>) -> PyResult<()> {
        let (reg, w) = self.live()?;
        let (id, param) = if event.is_instance_of::<pyo3::types::PyType>() {
            (id_of(event)?, std::ptr::null())
        } else {
            let id = id_of(&event.get_type().into_any())?;
            (id, value::data_ptr(event)? as *const c_void)
        };
        let _ = reg;
        // Notify for every id of the entity, so observers whose terms name
        // its components see the event (Flecs notifies each observer once).
        // With an empty list only `_` (Any) observers would match.
        let mut ids_array = ffi::entity_ids(w, self.id);
        let ids = sys::ecs_type_t {
            array: ids_array.as_mut_ptr(),
            count: ids_array.len() as i32,
        };
        let mut desc: sys::ecs_event_desc_t = unsafe { std::mem::zeroed() };
        desc.event = id;
        desc.ids = &ids;
        desc.entity = self.id;
        desc.const_param = param;
        desc.observable = w as *mut c_void;
        unsafe { sys::ecs_emit(w, &mut desc) };
        Ok(())
    }

    /// Makes this entity an instance of `prefab` (adds `(IsA, prefab)`).
    /// The prefab's components are copied in; prefab-specific systems now
    /// match it.
    fn is_a<'py>(slf: &Bound<'py, Self>, prefab: &Bound<'py, PyAny>) -> PyResult<Bound<'py, Self>> {
        let (reg, w) = slf.borrow().live()?;
        let p = id_of(prefab)?;
        if !ffi::has(w, p, unsafe { sys::EcsPrefab }) {
            return Err(PyTypeError::new_err(format!(
                "{} is not a prefab",
                ffi::path(w, p)
            )));
        }
        ffi::add(w, slf.borrow().id, ffi::pair(unsafe { sys::EcsIsA }, p));
        reg.structural_change();
        Ok(slf.clone())
    }

    /// Deletes the entity.
    fn destruct(&self) -> PyResult<()> {
        let (reg, w) = self.live()?;
        if let Some(sid) = reg.sid_of(self.id) {
            reg.sids.borrow_mut().remove(&sid);
        }
        unsafe { sys::ecs_delete(w, self.id) };
        reg.structural_change();
        Ok(())
    }

    fn enable(&self) -> PyResult<()> {
        self.set_enabled(true)
    }

    fn disable(&self) -> PyResult<()> {
        self.set_enabled(false)
    }

    #[getter]
    fn enabled(&self) -> PyResult<bool> {
        let (_, w) = self.live()?;
        Ok(ffi::enabled(w, self.id))
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .cast::<Entity>()
            .is_ok_and(|o| o.borrow().id == self.id)
    }

    fn __hash__(&self) -> u64 {
        self.id
    }

    fn __repr__(&self) -> String {
        match upgrade(&self.reg) {
            Ok(reg) if ffi::alive(reg.world, self.id) => {
                let sid = reg
                    .sid_of(self.id)
                    .map(|s| format!(" sid={s}"))
                    .unwrap_or_default();
                format!("Entity({}{sid})", ffi::path(reg.world, self.id))
            }
            _ => format!("Entity(#{} dead)", self.id & 0xFFFF_FFFF),
        }
    }
}

/// The world's seeded RNG (`sage_core::Rng`). The only randomness a
/// simulation may use.
#[pyclass(frozen, module = "sage")]
pub struct WorldRng {
    reg: RegId,
}

#[pymethods]
impl WorldRng {
    fn next_u64(&self) -> PyResult<u64> {
        Ok(upgrade(&self.reg)?.rng.borrow_mut().next_u64())
    }
    /// Uniform in [0, 1), 24 bits.
    fn next_f32(&self) -> PyResult<f32> {
        Ok(upgrade(&self.reg)?.rng.borrow_mut().next_f32())
    }
    /// Uniform in [lo, hi).
    fn range_f32(&self, lo: f32, hi: f32) -> PyResult<f32> {
        Ok(upgrade(&self.reg)?.rng.borrow_mut().range_f32(lo, hi))
    }
    fn next_bool(&self) -> PyResult<bool> {
        Ok(upgrade(&self.reg)?.rng.borrow_mut().next_bool())
    }
}

/// The native world handle. Scripts see it wrapped by `sage.ecs.World`.
#[pyclass(frozen, module = "sage", name = "NativeWorld")]
pub struct NativeWorld {
    pub reg: RegId,
}

impl NativeWorld {
    fn reg(&self) -> PyResult<Rc<Registry>> {
        upgrade(&self.reg)
    }

    fn fresh_child(&self, parent: u64, name: &str) -> PyResult<u64> {
        let w = self.reg()?.world;
        if ffi::lookup_child(w, parent, name) != 0 {
            return Err(PyValueError::new_err(format!(
                "{}.{name} is already defined",
                ffi::path(w, parent)
            )));
        }
        Ok(ffi::named_child(w, parent, name))
    }

    fn field_tuples(reg: &Registry, id: u64) -> FieldTuples {
        match &reg.types.borrow()[&id].layout {
            Layout::Struct(fields) => fields
                .iter()
                .map(|f| (f.name.clone(), f.kind.name().to_string(), f.offset))
                .collect(),
            _ => vec![],
        }
    }
}

unsafe extern "C-unwind" fn opaque_ctor(
    ptr: *mut c_void,
    count: i32,
    _ti: *const sys::ecs_type_info_t,
) {
    unsafe { std::ptr::write_bytes(ptr as *mut *mut pyo3::ffi::PyObject, 0, count as usize) };
}

unsafe extern "C-unwind" fn opaque_dtor(
    ptr: *mut c_void,
    count: i32,
    _ti: *const sys::ecs_type_info_t,
) {
    Python::attach(|_py| {
        let p = ptr as *mut *mut pyo3::ffi::PyObject;
        for i in 0..count as usize {
            unsafe {
                let o = *p.add(i);
                if !o.is_null() {
                    pyo3::ffi::Py_DecRef(o);
                    *p.add(i) = std::ptr::null_mut();
                }
            }
        }
    });
}

unsafe extern "C-unwind" fn opaque_copy(
    dst: *mut c_void,
    src: *const c_void,
    count: i32,
    _ti: *const sys::ecs_type_info_t,
) {
    Python::attach(|py| {
        let d = dst as *mut *mut pyo3::ffi::PyObject;
        let s = src as *const *mut pyo3::ffi::PyObject;
        let deepcopy = py.import("copy").and_then(|m| m.getattr("deepcopy"));
        for i in 0..count as usize {
            unsafe {
                let src_o = *s.add(i);
                let new = if src_o.is_null() {
                    std::ptr::null_mut()
                } else {
                    let b = Bound::from_borrowed_ptr(py, src_o);
                    match deepcopy.as_ref().map(|f| f.call1((b,))) {
                        Ok(Ok(c)) => c.into_ptr(),
                        _ => {
                            // A value that cannot be copied is shared instead.
                            pyo3::ffi::Py_IncRef(src_o);
                            src_o
                        }
                    }
                };
                let old = *d.add(i);
                *d.add(i) = new;
                if !old.is_null() {
                    pyo3::ffi::Py_DecRef(old);
                }
            }
        }
    });
}

unsafe extern "C-unwind" fn opaque_move(
    dst: *mut c_void,
    src: *mut c_void,
    count: i32,
    _ti: *const sys::ecs_type_info_t,
) {
    Python::attach(|_py| {
        let d = dst as *mut *mut pyo3::ffi::PyObject;
        let s = src as *mut *mut pyo3::ffi::PyObject;
        for i in 0..count as usize {
            unsafe {
                let old = *d.add(i);
                *d.add(i) = *s.add(i);
                *s.add(i) = std::ptr::null_mut();
                if !old.is_null() {
                    pyo3::ffi::Py_DecRef(old);
                }
            }
        }
    });
}

#[pymethods]
impl NativeWorld {
    #[getter]
    fn script_module(&self) -> PyResult<u64> {
        Ok(self.reg()?.script_module.get())
    }

    #[getter]
    fn sid_type(&self) -> PyResult<u64> {
        Ok(self.reg()?.sid_type)
    }

    #[getter]
    fn rng(&self) -> PyResult<WorldRng> {
        Ok(WorldRng { reg: self.reg })
    }

    fn _path(&self, id: u64) -> PyResult<String> {
        Ok(ffi::path(self.reg()?.world, id))
    }

    fn _entity(&self, id: u64) -> PyResult<Entity> {
        Ok(Entity::new(self.reg, id))
    }

    /// Defines a reflected struct component under the script's flecs module.
    /// Returns (id, [(name, kind, offset)], size).
    #[pyo3(signature = (name, fields, base=None))]
    fn _define_struct(
        &self,
        name: &str,
        fields: Vec<(String, String)>,
        base: Option<u64>,
    ) -> PyResult<(u64, FieldTuples, usize)> {
        let reg = self.reg()?;
        let w = reg.world;
        if fields.len() > 32 {
            return Err(PyValueError::new_err(
                "a component can have at most 32 fields",
            ));
        }
        let kinds = fields
            .iter()
            .map(|(n, k)| {
                Kind::from_name(k)
                    .ok_or_else(|| PyTypeError::new_err(format!("{name}.{n}: unknown kind {k}")))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let e = self.fresh_child(reg.script_module.get(), name)?;
        let names: Vec<_> = fields.iter().map(|(n, _)| ffi::cstring(n)).collect();
        let mut desc: sys::ecs_struct_desc_t = unsafe { std::mem::zeroed() };
        desc.entity = e;
        for (i, k) in kinds.iter().enumerate() {
            desc.members[i].name = names[i].as_ptr();
            desc.members[i].type_ = k.type_id();
        }
        let id = unsafe { sys::ecs_struct_init(w, &desc) };
        if id == 0 {
            return Err(PyRuntimeError::new_err(format!(
                "Flecs rejected component {name}"
            )));
        }
        // Every Python struct component can be a base: queries for it then
        // also match components that inherit from it (verified at the pin:
        // tests/flecs_assumptions.rs).
        ffi::add(w, id, unsafe { sys::EcsInheritable });
        if let Some(base) = base {
            let base_fields = Self::field_tuples(&reg, base);
            reg.register_reflected(id).map_err(PyTypeError::new_err)?;
            let own = Self::field_tuples(&reg, id);
            if own.len() < base_fields.len() || own[..base_fields.len()] != base_fields[..] {
                return Err(PyTypeError::new_err(format!(
                    "{name}: the layout of the base component's fields is not a prefix"
                )));
            }
            ffi::add(w, id, ffi::pair(unsafe { sys::EcsIsA }, base));
        }
        reg.register_reflected(id).map_err(PyTypeError::new_err)?;
        let size = reg.types.borrow()[&id].size;
        Ok((id, Self::field_tuples(&reg, id), size))
    }

    fn _define_tag(&self, name: &str) -> PyResult<u64> {
        let reg = self.reg()?;
        let e = self.fresh_child(reg.script_module.get(), name)?;
        reg.types.borrow_mut().insert(
            e,
            TypeInfo {
                path: ffi::path(reg.world, e),
                size: 0,
                layout: Layout::Tag,
                class: None,
            },
        );
        Ok(e)
    }

    fn _define_opaque(&self, name: &str) -> PyResult<u64> {
        let reg = self.reg()?;
        let w = reg.world;
        let e = self.fresh_child(reg.script_module.get(), name)?;
        let size = std::mem::size_of::<*mut pyo3::ffi::PyObject>();
        let desc = sys::ecs_component_desc_t {
            _canary: 0,
            entity: e,
            type_: sys::ecs_type_info_t {
                size: size as i32,
                alignment: std::mem::align_of::<*mut pyo3::ffi::PyObject>() as i32,
                hooks: unsafe { std::mem::zeroed() },
                component: 0,
                name: std::ptr::null(),
            },
        };
        let id = unsafe { sys::ecs_component_init(w, &desc) };
        let mut hooks: sys::ecs_type_hooks_t = unsafe { std::mem::zeroed() };
        hooks.ctor = Some(opaque_ctor);
        hooks.dtor = Some(opaque_dtor);
        hooks.copy = Some(opaque_copy);
        hooks.move_ = Some(opaque_move);
        unsafe { sys::ecs_set_hooks_id(w, id, &hooks) };
        reg.types.borrow_mut().insert(
            id,
            TypeInfo {
                path: ffi::path(w, id),
                size,
                layout: Layout::Opaque,
                class: None,
            },
        );
        Ok(id)
    }

    /// Reflection data for a native component: (id, path, size, fields).
    fn _reflect(&self, id: u64) -> PyResult<(u64, String, usize, FieldTuples)> {
        let reg = self.reg()?;
        reg.register_reflected(id).map_err(PyTypeError::new_err)?;
        let t = &reg.types.borrow()[&id];
        let fields = match &t.layout {
            Layout::Struct(fs) => fs
                .iter()
                .map(|f: &FieldInfo| (f.name.clone(), f.kind.name().to_string(), f.offset))
                .collect(),
            _ => vec![],
        };
        Ok((id, t.path.clone(), t.size, fields))
    }

    fn _lookup(&self, path: &str) -> PyResult<u64> {
        Ok(ffi::lookup_path(self.reg()?.world, path))
    }

    fn _bind_class(&self, id: u64, cls: Py<PyAny>) -> PyResult<()> {
        let reg = self.reg()?;
        let mut types = reg.types.borrow_mut();
        let t = types
            .get_mut(&id)
            .ok_or_else(|| PyKeyError::new_err(format!("unknown component {id}")))?;
        t.class = Some(cls);
        Ok(())
    }

    fn _class(&self, py: Python<'_>, id: u64) -> PyResult<Option<Py<PyAny>>> {
        let _ = py;
        Ok(self.reg()?.class_of(id))
    }

    fn _field(&self, name: String, kind: &str, offset: usize) -> PyResult<Field> {
        let kind = Kind::from_name(kind).ok_or_else(|| PyTypeError::new_err("unknown kind"))?;
        Ok(Field::new(name, offset, kind, self.reg))
    }

    /// Creates a flecs module under `parent` (default: the script's module).
    #[pyo3(signature = (name, parent=None))]
    fn _module(&self, name: &str, parent: Option<u64>) -> PyResult<u64> {
        let reg = self.reg()?;
        let parent = parent.unwrap_or(reg.script_module.get());
        let e = self.fresh_child(parent, name)?;
        ffi::add(reg.world, e, unsafe { sys::EcsModule });
        reg.modules.borrow_mut().push(e);
        Ok(e)
    }

    /// A pipeline phase entity from a name ("OnUpdate"), a path or an Entity.
    fn _resolve_phase(&self, phase: &Bound<'_, PyAny>) -> PyResult<u64> {
        let reg = self.reg()?;
        if let Ok(s) = phase.extract::<String>() {
            if let Some(p) = ffi::builtin_phase(&s) {
                return Ok(p);
            }
            let p = ffi::lookup_path(reg.world, &s);
            if p == 0 {
                return Err(PyValueError::new_err(format!("unknown phase {s:?}")));
            }
            return Ok(p);
        }
        id_of(phase)
    }

    /// An event entity from "OnAdd"/"OnSet"/"OnRemove" or a path.
    fn _resolve_event(&self, name: &str) -> PyResult<u64> {
        if let Some(e) = ffi::builtin_event(name) {
            return Ok(e);
        }
        let e = ffi::lookup_path(self.reg()?.world, name);
        if e == 0 {
            return Err(PyValueError::new_err(format!("unknown event {name:?}")));
        }
        Ok(e)
    }

    /// Creates a pipeline phase that runs after `after`.
    fn _phase(&self, name: &str, after: u64) -> PyResult<u64> {
        let reg = self.reg()?;
        let w = reg.world;
        let e = self.fresh_child(reg.script_module.get(), name)?;
        ffi::add(w, e, unsafe { sys::EcsPhase });
        ffi::add(w, e, ffi::pair(unsafe { sys::EcsDependsOn }, after));
        Ok(e)
    }

    /// A new entity with a StableId (the next free one unless given).
    #[pyo3(signature = (name=None, sid=None))]
    fn entity(&self, name: Option<&str>, sid: Option<u64>) -> PyResult<Entity> {
        let reg = self.reg()?;
        let w = reg.world;
        let e = match name {
            Some(n) => self.fresh_child(0, n)?,
            None => unsafe { sys::ecs_new(w) },
        };
        reg.assign_sid(e, sid).map_err(PyValueError::new_err)?;
        reg.structural_change();
        Ok(Entity::new(reg.id, e))
    }

    /// A prefab under the script's flecs module, optionally derived from
    /// `base` (`(IsA, base)`). Prefabs have a StableId (not copied to their
    /// instances) and are skipped by queries and systems.
    #[pyo3(signature = (name, base=None, sid=None))]
    fn _prefab(&self, name: &str, base: Option<u64>, sid: Option<u64>) -> PyResult<u64> {
        let reg = self.reg()?;
        let w = reg.world;
        let prefab = unsafe { sys::EcsPrefab };
        if let Some(b) = base
            && !ffi::has(w, b, prefab)
        {
            return Err(PyTypeError::new_err(format!(
                "{} is not a prefab",
                ffi::path(w, b)
            )));
        }
        let e = self.fresh_child(reg.script_module.get(), name)?;
        ffi::add(w, e, prefab);
        if let Some(b) = base {
            ffi::add(w, e, ffi::pair(unsafe { sys::EcsIsA }, b));
        }
        reg.assign_sid(e, sid).map_err(PyValueError::new_err)?;
        reg.structural_change();
        Ok(e)
    }

    /// The entity with StableId `sid`, or None.
    fn lookup(&self, sid: u64) -> PyResult<Option<Entity>> {
        let reg = self.reg()?;
        Ok(reg.entity_of_sid(sid).map(|e| Entity::new(reg.id, e)))
    }

    /// The entity at a dotted path, or None.
    fn lookup_path(&self, path: &str) -> PyResult<Option<Entity>> {
        let reg = self.reg()?;
        let e = ffi::lookup_path(reg.world, path);
        Ok((e != 0).then(|| Entity::new(reg.id, e)))
    }

    /// Sets a singleton (a component stored on its own component entity).
    fn set(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let id = id_of(&value.get_type().into_any())?;
        set_value(&self.reg()?, id, value)
    }

    /// A singleton's value (a view inside callbacks), or None.
    fn get<'py>(
        &self,
        py: Python<'py>,
        cls: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let id = id_of(cls)?;
        get_value(py, &self.reg()?, id, cls)
    }

    /// Instantiates a native flecs module registered by the host (for
    /// example collision), with its systems in `phase`, under `parent`.
    #[pyo3(signature = (name, phase, parent=None))]
    fn _import_native(&self, name: &str, phase: u64, parent: Option<u64>) -> PyResult<u64> {
        let reg = self.reg()?;
        let parent = parent.unwrap_or(reg.script_module.get());
        let e = {
            let mods = reg.native_modules.borrow();
            let f = mods.get(name).ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "no native module {name:?}; known: {:?}",
                    mods.keys().collect::<Vec<_>>()
                ))
            })?;
            f(reg.world, phase, parent).map_err(PyRuntimeError::new_err)?
        };
        reg.modules.borrow_mut().push(e);
        reg.structural_change();
        Ok(e)
    }

    /// Creates a system or observer. Called by `sage.ecs`; see there.
    #[allow(clippy::too_many_arguments)]
    fn _create(
        &self,
        kind: &str,
        name: &str,
        parent: Option<u64>,
        phase: u64,
        expr: &str,
        n_fixed: usize,
        params: &Bound<'_, PyList>,
        func: Py<PyAny>,
        immediate: bool,
        events: Vec<u64>,
        event_type: u64,
    ) -> PyResult<Entity> {
        let reg = self.reg()?;
        let parent = parent.unwrap_or(reg.script_module.get());
        let params = params
            .iter()
            .map(|p| {
                let t = p.cast::<PyTuple>()?;
                Ok((
                    t.get_item(0)?.extract::<String>()?,
                    t.get_item(1)?.extract::<Option<usize>>()?,
                    t.get_item(2)?.extract::<String>()?,
                ))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let e = self.fresh_child(parent, name)?;
        let spec = crate::callbacks::Spec {
            kind,
            entity: e,
            phase,
            expr,
            n_fixed,
            params,
            func,
            immediate,
            events,
            event_type,
        };
        crate::callbacks::create(&reg, spec).map_err(|msg| {
            unsafe { sys::ecs_delete(reg.world, e) };
            PyValueError::new_err(msg)
        })?;
        Ok(Entity::new(reg.id, e))
    }
}
