//! The block editor's Python half (`sage.blocks`, design doc 4 and M4).
//!
//! `python/sage/blocks.py` does the work with `libcst`, which is not part of
//! the standard library: `python -m pip install --target python/vendor/pyXY -r
//! python/requirements.txt` (XY is the embedded Python's version, such as
//! 312) puts it where [`install`] looks (override the base directory with
//! the `SAGE_PY_VENDOR` environment variable). The interpreter is isolated,
//! so nothing else on the machine is visible to scripts.

use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::ffi::CString;
use std::sync::OnceLock;

pub const BLOCKS_SOURCE: &str = include_str!("../../../python/sage/blocks.py");

/// Where `libcst` lives: `SAGE_PY_VENDOR`, else `python/vendor` in the
/// repository this crate was built from.
pub fn vendor_dir() -> String {
    std::env::var("SAGE_PY_VENDOR")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../python/vendor").to_string())
}

static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();

/// Makes `sage.blocks` importable (once per process).
pub fn install() -> Result<(), String> {
    sage_script::init().map_err(|e| e.0)?;
    INSTALLED
        .get_or_init(|| {
            Python::attach(|py| -> PyResult<()> {
                let sys = py.import("sys")?;
                let v = sys.getattr("version_info")?;
                let dir = format!(
                    "{}/py{}{}",
                    vendor_dir(),
                    v.get_item(0)?.extract::<u32>()?,
                    v.get_item(1)?.extract::<u32>()?
                );
                sys.getattr("path")?.call_method1("insert", (0, &dir))?;
                if py.import("libcst").is_err() {
                    return Err(pyo3::exceptions::PyImportError::new_err(format!(
                        "libcst is not installed in {dir}: run `python -m pip install --target {dir} -r python/requirements.txt` with the Python the engine embeds"
                    )));
                }
                let code = CString::new(BLOCKS_SOURCE).unwrap();
                let m = PyModule::from_code(py, &code, c"<sage.blocks>", c"sage.blocks")?;
                sys.getattr("modules")?.set_item("sage.blocks", &m)?;
                sys.getattr("modules")?.set_item("blocks", &m)?;
                sage_script::package(py)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.0))?
                    .setattr("blocks", &m)?;
                Ok(())
            })
            .map_err(|e| Python::attach(|py| sage_script::describe(py, &e).0))
        })
        .clone()
}

/// Runs a Python test module's `run()` and returns the failures as text
/// (empty when all pass).
pub fn run_python_tests(name: &str, source: &str) -> Result<String, String> {
    install()?;
    Python::attach(|py| -> PyResult<String> {
        let code = CString::new(source).unwrap();
        let file = CString::new(format!("<{name}>")).unwrap();
        let modname = CString::new(name).unwrap();
        let m = PyModule::from_code(py, &code, &file, &modname)?;
        py.import("sys")?.getattr("modules")?.set_item(name, &m)?;
        m.getattr("run")?.call0()?.extract()
    })
    .map_err(|e| Python::attach(|py| sage_script::describe(py, &e).0))
}

/// The source with every statement in a function reporting its line as it
/// runs (`sage.blocks.instrument`), for glowing blocks while debugging.
pub fn instrument(source: &str) -> Result<String, String> {
    install()?;
    Python::attach(|py| -> PyResult<String> {
        let m = py.import("sage.blocks")?;
        m.call_method0("install_glow")?;
        m.call_method1("instrument", (source,))?.extract()
    })
    .map_err(|e| Python::attach(|py| sage_script::describe(py, &e).0))
}
