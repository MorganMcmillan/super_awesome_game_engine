//! Component values and views.
//!
//! Every reflected component class derives from `Value`. An instance either
//! owns its bytes (`Position(x=1)` built in Python) or is a *view* that
//! points into Flecs storage (a system parameter, `entity.get(Position)`).
//! A view is only valid while the callback that produced it runs and no
//! structural change has moved component data; after that every access
//! raises `ReferenceError` instead of touching freed memory.

use std::rc::Rc;

use pyo3::exceptions::{PyAttributeError, PyReferenceError, PyTypeError, PyValueError};
use pyo3::prelude::*;

use crate::api::Entity;
use crate::registry::{Kind, RegId, Registry, Validity};

#[pyclass(subclass, module = "sage")]
pub struct Value {
    ptr: *mut u8,
    buf: Option<Vec<u64>>,
    view: Option<(RegId, Validity)>,
    readonly: bool,
}

#[pymethods]
impl Value {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    fn new(_args: &Bound<'_, PyAny>, _kwargs: Option<&Bound<'_, PyAny>>) -> Self {
        Value {
            ptr: std::ptr::null_mut(),
            buf: None,
            view: None,
            readonly: false,
        }
    }

    /// Gives a fresh value its own zeroed storage.
    fn _alloc(&mut self, size: usize) {
        let mut buf = vec![0u64; size.div_ceil(8).max(1)];
        self.ptr = buf.as_mut_ptr() as *mut u8;
        self.buf = Some(buf);
        self.view = None;
        self.readonly = false;
    }

    /// True for a view into the world, false for a value that owns its data.
    #[getter]
    fn is_view(&self) -> bool {
        self.view.is_some()
    }

    /// True while the object can be read.
    #[getter]
    fn is_valid(&self) -> bool {
        self.check(false).is_ok()
    }
}

impl Value {
    /// The data pointer, if it may be used for reading (or writing).
    pub fn check(&self, write: bool) -> PyResult<*mut u8> {
        if self.ptr.is_null() {
            return Err(PyValueError::new_err(
                "component value has no data (construct it with its class)",
            ));
        }
        if let Some((reg, v)) = &self.view {
            let ok = crate::registry::lookup(*reg).is_some_and(|r| r.is_valid(v));
            if !ok {
                return Err(PyReferenceError::new_err(
                    "this component view is no longer valid: views only live until the \
                     system or observer call that produced them returns, or until a \
                     structural change (add, remove, new entity) moves component data. \
                     Copy it with .copy() to keep the values",
                ));
            }
        }
        if write && self.readonly {
            return Err(PyAttributeError::new_err(
                "this view is read-only (the query term is [in] or matched through traversal)",
            ));
        }
        Ok(self.ptr)
    }
}

/// Makes a Python object for component data at `ptr`: a view when inside a
/// callback scope, else an owned copy.
pub fn make_object<'py>(
    py: Python<'py>,
    reg: &Rc<Registry>,
    class: &Bound<'py, PyAny>,
    ptr: *mut u8,
    size: usize,
    readonly: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let obj = class.call_method1(pyo3::intern!(py, "__new__"), (class,))?;
    {
        let v = obj.cast::<Value>()?;
        let mut v = v.borrow_mut();
        match reg.stamp() {
            Some(stamp) => {
                v.ptr = ptr;
                v.view = Some((reg.id, stamp));
                v.readonly = readonly;
            }
            None => {
                v._alloc(size);
                unsafe { std::ptr::copy_nonoverlapping(ptr, v.ptr, size) };
            }
        }
    }
    Ok(obj)
}

/// Copies `obj`'s bytes out, for `set`. `obj` must be a Value.
pub fn data_ptr(obj: &Bound<'_, PyAny>) -> PyResult<*mut u8> {
    let v = obj
        .cast::<Value>()
        .map_err(|_| PyTypeError::new_err("expected a component value"))?;
    v.borrow().check(false)
}

/// A descriptor for one reflected field.
#[pyclass(frozen, module = "sage")]
pub struct Field {
    name: String,
    offset: usize,
    kind: Kind,
    reg: RegId,
}

impl Field {
    pub fn new(name: String, offset: usize, kind: Kind, reg: RegId) -> Self {
        Field {
            name,
            offset,
            kind,
            reg,
        }
    }
}

#[pymethods]
impl Field {
    fn __get__(
        slf: &Bound<'_, Self>,
        obj: Option<&Bound<'_, PyAny>>,
        _ty: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let Some(obj) = obj.filter(|o| !o.is_none()) else {
            return Ok(slf.clone().into_any().unbind());
        };
        let f = slf.borrow();
        let base = obj.cast::<Value>()?.borrow().check(false)?;
        let p = unsafe { base.add(f.offset) };
        unsafe { read(py, f.kind, p, f.reg) }
    }

    fn __set__(&self, obj: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let base = obj.cast::<Value>()?.borrow().check(true)?;
        let p = unsafe { base.add(self.offset) };
        unsafe { write(self.kind, p, value) }.map_err(|e| {
            PyTypeError::new_err(format!("field {} ({}): {e}", self.name, self.kind.name()))
        })
    }

    fn __delete__(&self, _obj: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(PyAttributeError::new_err(format!(
            "cannot delete component field {}",
            self.name
        )))
    }

    #[getter]
    fn kind(&self) -> &'static str {
        self.kind.name()
    }

    #[getter]
    fn offset(&self) -> usize {
        self.offset
    }

    fn __repr__(&self) -> String {
        format!(
            "<field {}: {} at {}>",
            self.name,
            self.kind.name(),
            self.offset
        )
    }
}

/// # Safety
/// `p` must point to a valid, aligned value of `kind`.
pub unsafe fn read(py: Python<'_>, kind: Kind, p: *const u8, reg: RegId) -> PyResult<Py<PyAny>> {
    unsafe {
        Ok(match kind {
            Kind::Bool => (*p != 0).into_pyobject(py)?.to_owned().into_any().unbind(),
            Kind::U8 => (*p).into_pyobject(py)?.into_any().unbind(),
            Kind::I32 => (*(p as *const i32)).into_pyobject(py)?.into_any().unbind(),
            Kind::U32 => (*(p as *const u32)).into_pyobject(py)?.into_any().unbind(),
            Kind::I64 => (*(p as *const i64)).into_pyobject(py)?.into_any().unbind(),
            Kind::U64 => (*(p as *const u64)).into_pyobject(py)?.into_any().unbind(),
            Kind::F32 => (*(p as *const f32) as f64)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            Kind::F64 => (*(p as *const f64)).into_pyobject(py)?.into_any().unbind(),
            Kind::Entity => {
                let id = *(p as *const u64);
                if id == 0 {
                    py.None()
                } else {
                    Py::new(py, Entity::new(reg, id))?.into_any()
                }
            }
        })
    }
}

/// # Safety
/// `p` must point to a valid, aligned, writable value of `kind`.
pub unsafe fn write(kind: Kind, p: *mut u8, value: &Bound<'_, PyAny>) -> PyResult<()> {
    unsafe {
        match kind {
            Kind::Bool => *p = value.extract::<bool>()? as u8,
            Kind::U8 => *p = value.extract::<u8>()?,
            Kind::I32 => *(p as *mut i32) = value.extract()?,
            Kind::U32 => *(p as *mut u32) = value.extract()?,
            Kind::I64 => *(p as *mut i64) = value.extract()?,
            Kind::U64 => *(p as *mut u64) = value.extract()?,
            Kind::F32 => *(p as *mut f32) = value.extract::<f64>()? as f32,
            Kind::F64 => *(p as *mut f64) = value.extract()?,
            Kind::Entity => {
                *(p as *mut u64) = if value.is_none() {
                    0
                } else if let Ok(e) = value.cast::<Entity>() {
                    e.borrow().id
                } else {
                    return Err(PyTypeError::new_err("expected an Entity or None"));
                }
            }
        }
    }
    Ok(())
}

// SAFETY: a `Value` is either an owned buffer (usable from any thread) or a
// view whose every access first checks `registry::lookup`, which only
// succeeds on the thread that owns the world. So the raw pointer is never
// dereferenced from another thread; moving or dropping the object is fine.
unsafe impl Send for Value {}
// SAFETY: as above; shared access only reads the pointer after the same
// check, and mutation goes through PyO3's borrow flag (GIL build).
unsafe impl Sync for Value {}

/// Points an existing view object at another row (a reused system argument).
pub fn rebind(
    obj: &Bound<'_, PyAny>,
    reg: &Rc<Registry>,
    ptr: *mut u8,
    readonly: bool,
) -> PyResult<()> {
    let v = obj.cast::<Value>()?;
    let mut v = v.borrow_mut();
    let stamp = reg
        .stamp()
        .ok_or_else(|| PyValueError::new_err("rebind outside a callback scope"))?;
    v.ptr = ptr;
    v.buf = None;
    v.view = Some((reg.id, stamp));
    v.readonly = readonly;
    Ok(())
}
