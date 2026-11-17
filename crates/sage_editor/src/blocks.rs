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

/// The editor's other Python modules, loaded after `sage.blocks`: `(name,
/// source)`. Each is importable as `sage.<name>` and, like `blocks`, by its
/// bare name (the tests import them that way).
pub const MODULES: &[(&str, &str)] = &[
    (
        "commands",
        include_str!("../../../python/sage/commands.py"),
    ),
    (
        "provenance",
        include_str!("../../../python/sage/provenance.py"),
    ),
];

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
                let package = sage_script::package(py)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.0))?;
                let modules = sys.getattr("modules")?;
                for (name, source) in [("blocks", BLOCKS_SOURCE)].iter().chain(MODULES) {
                    let full = format!("{}.{name}", sage_script::PY_PACKAGE);
                    let code = CString::new(*source).unwrap();
                    let file = CString::new(format!("<{full}>")).unwrap();
                    let m = PyModule::from_code(py, &code, &file, &CString::new(&*full).unwrap())?;
                    modules.set_item(&full, &m)?;
                    modules.set_item(name, &m)?;
                    package.setattr(*name, &m)?;
                }
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
