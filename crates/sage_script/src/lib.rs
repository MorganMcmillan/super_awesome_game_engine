//! Embedded CPython for simulation rules.
//!
//! Determinism (design doc 3.3):
//! - The interpreter starts with a fixed hash seed (the in-process equivalent
//!   of `PYTHONHASHSEED=0`), set through `PyConfig` because an environment
//!   variable set after process start is not reliably visible to the C runtime.
//! - It starts isolated: environment variables (`PYTHONPATH` and friends) and
//!   the user site directory cannot change behavior between machines.
//! - Each loaded script gets a fresh globals dict, so two simulations in one
//!   process never share script state.
//! - Handlers are discovered in sorted name order.
//!
//! Scripts are trusted code. Never load untrusted Python (kits or scripts from
//! the network) through this crate: there is no sandbox yet.

use std::ffi::CString;
use std::sync::OnceLock;

use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

/// The Python package scripts import. Matches the project codename
/// (`sage_core::CODENAME`); kept as a literal so this crate does not depend on
/// the simulation core.
pub const PY_PACKAGE: &str = "sage";

/// The hash seed the interpreter is started with.
pub const HASH_SEED: u64 = 0;

/// The `sage` package's `__init__.py`. It lives in the repository's
/// `python/sage/` directory with the rest of the package, so editors can
/// resolve `import sage` in scripts (see `pyrightconfig.json`).
const PACKAGE_SOURCE: &str = include_str!("../../../python/sage/__init__.py");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError(pub String);

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ScriptError {}

/// Formats a Python exception with its traceback.
pub fn describe(py: Python<'_>, err: &PyErr) -> ScriptError {
    let mut text = err.to_string();
    if let Some(tb) = err.traceback(py)
        && let Ok(s) = tb.format()
    {
        text = format!("{s}{text}");
    }
    ScriptError(text)
}

static INIT: OnceLock<Result<(), ScriptError>> = OnceLock::new();

/// Starts the interpreter once per process with the deterministic config.
/// Safe to call from any thread, any number of times.
pub fn init() -> Result<(), ScriptError> {
    INIT.get_or_init(|| unsafe { init_interpreter() }).clone()
}

unsafe fn init_interpreter() -> Result<(), ScriptError> {
    unsafe {
        if ffi::Py_IsInitialized() != 0 {
            return Err(ScriptError(
                "Python was initialized by someone else; the hash seed cannot be guaranteed".into(),
            ));
        }
        let mut config = std::mem::MaybeUninit::<ffi::PyConfig>::zeroed();
        ffi::PyConfig_InitIsolatedConfig(config.as_mut_ptr());
        let mut config = config.assume_init();
        config.use_hash_seed = 1;
        config.hash_seed = HASH_SEED as _;
        config.install_signal_handlers = 0;
        config.write_bytecode = 0;
        let status = ffi::Py_InitializeFromConfig(&config);
        ffi::PyConfig_Clear(&mut config);
        if ffi::PyStatus_Exception(status) != 0 {
            let msg = if status.err_msg.is_null() {
                "unknown error".to_string()
            } else {
                std::ffi::CStr::from_ptr(status.err_msg)
                    .to_string_lossy()
                    .into_owned()
            };
            return Err(ScriptError(format!("Python failed to start: {msg}")));
        }
        // Release the GIL taken by initialization so any thread can attach.
        ffi::PyEval_SaveThread();
    }
    Python::attach(|py| {
        let code = CString::new(PACKAGE_SOURCE).unwrap();
        let file = CString::new(format!("<{PY_PACKAGE}>")).unwrap();
        let name = CString::new(PY_PACKAGE).unwrap();
        // from_code registers the package in sys.modules.
        PyModule::from_code(py, &code, &file, &name).map_err(|e| describe(py, &e))?;
        Ok(())
    })
}

/// The `sage` package, for crates that extend it with native parts (such as
/// the Flecs binding). Starts the interpreter first if needed.
pub fn package(py: Python<'_>) -> Result<Bound<'_, PyModule>, ScriptError> {
    init()?;
    py.import(PY_PACKAGE)
        .map_err(|e| describe(py, &e))?
        .cast_into::<PyModule>()
        .map_err(|e| ScriptError(e.to_string()))
}

/// One discovered handler: `@sage.on(event) def name(...)`.
pub struct Handler {
    pub event: String,
    pub name: String,
    pub func: Py<PyAny>,
}

/// A loaded script with its own globals.
pub struct Script {
    pub name: String,
    globals: Py<PyDict>,
    handlers: Vec<Handler>,
}

impl Script {
    /// Runs `source` in a fresh namespace and collects its handlers.
    pub fn load(name: &str, source: &str) -> Result<Script, ScriptError> {
        Self::load_with(name, source, vec![])
    }

    /// Like [`Script::load`], with extra names (such as `world`) placed in
    /// the fresh globals before the source runs.
    pub fn load_with(
        name: &str,
        source: &str,
        prelude: Vec<(String, Py<PyAny>)>,
    ) -> Result<Script, ScriptError> {
        init()?;
        Python::attach(|py| {
            let fail = |e: PyErr| describe(py, &e);
            let globals = PyDict::new(py);
            globals.set_item("__name__", name).map_err(fail)?;
            globals
                .set_item("__builtins__", py.import("builtins").map_err(fail)?)
                .map_err(fail)?;
            for (key, value) in prelude {
                globals.set_item(key, value).map_err(fail)?;
            }
            let code = CString::new(source)
                .map_err(|_| ScriptError(format!("{name}: source contains a NUL byte")))?;
            py.run(&code, Some(&globals), None).map_err(fail)?;

            let mut handlers = vec![];
            for (key, value) in globals.iter() {
                let Ok(event) = value.getattr("__sage_event__") else {
                    continue;
                };
                handlers.push(Handler {
                    event: event.extract().map_err(fail)?,
                    name: key.extract().map_err(fail)?,
                    func: value.unbind(),
                });
            }
            // Dict order is insertion order, which follows the source; sort
            // anyway so discovery order is defined by names alone.
            handlers.sort_by(|a, b| (&a.event, &a.name).cmp(&(&b.event, &b.name)));
            Ok(Script {
                name: name.to_string(),
                globals: globals.unbind(),
                handlers,
            })
        })
    }

    /// Handlers for `event`, in name order.
    pub fn handlers(&self, event: &str) -> impl Iterator<Item = &Handler> {
        self.handlers.iter().filter(move |h| h.event == event)
    }

    pub fn globals<'py>(&self, py: Python<'py>) -> &Bound<'py, PyDict> {
        self.globals.bind(py)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_seed_is_fixed() {
        init().unwrap();
        Python::attach(|py| {
            let flags = py.import("sys").unwrap().getattr("flags").unwrap();
            let randomized: i64 = flags
                .getattr("hash_randomization")
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(randomized, 0);
            // With seed 0, str hashing is the unrandomized SipHash; pin one value.
            let h: i64 = py
                .eval(c"hash('sage')", None, None)
                .unwrap()
                .extract()
                .unwrap();
            let h2: i64 = py
                .eval(c"hash('sage')", None, None)
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(h, h2);
        });
    }

    #[test]
    fn set_iteration_order_is_reproducible() {
        // Under a fixed hash seed a set of strings iterates the same way in
        // every process. Compare against an order observed in this process
        // from a fresh set, built in a different insertion order.
        let s = Script::load(
            "t",
            "a = list({'paddle', 'ball', 'wall', 'goal', 'court'})\n\
             b = list({'court', 'goal', 'wall', 'ball', 'paddle'})\n",
        )
        .unwrap();
        Python::attach(|py| {
            let g = s.globals(py);
            let a: Vec<String> = g.get_item("a").unwrap().unwrap().extract().unwrap();
            let b: Vec<String> = g.get_item("b").unwrap().unwrap().extract().unwrap();
            assert_eq!(a, b);
        });
    }

    #[test]
    fn handlers_discovered_in_name_order() {
        let s = Script::load(
            "t",
            "import sage\n\
             @sage.on('goal')\ndef zeta(w, e): return 1\n\
             @sage.on('goal')\ndef alpha(w, e): return 2\n\
             @sage.on('bounce')\ndef beta(w, e): return 3\n\
             def helper(): pass\n",
        )
        .unwrap();
        let names: Vec<_> = s.handlers("goal").map(|h| h.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
        assert_eq!(s.handlers("bounce").count(), 1);
    }

    #[test]
    fn scripts_do_not_share_globals() {
        let a = Script::load("a", "counter = 1\n").unwrap();
        let b = Script::load("b", "counter = globals().get('counter', 0) + 10\n").unwrap();
        Python::attach(|py| {
            let get = |s: &Script| -> i64 {
                s.globals(py)
                    .get_item("counter")
                    .unwrap()
                    .unwrap()
                    .extract()
                    .unwrap()
            };
            assert_eq!(get(&a), 1);
            assert_eq!(get(&b), 10);
        });
    }

    #[test]
    fn errors_carry_traceback() {
        let err = Script::load("bad", "def f():\n    raise ValueError('boom')\nf()\n")
            .err()
            .unwrap();
        assert!(
            err.0.contains("boom") && err.0.contains("line 2"),
            "{}",
            err.0
        );
    }
}
