//! Spike S4: PyO3 embedding cost and behavior. Write-up: docs/spikes/S4.md.
//!
//! Integrates `pos += vel * dt` for N entities from Python, three ways:
//!   A. one Python call per entity (beginner style)
//!   B. one Python call per tick over memoryviews of the Rust columns,
//!      looping in Python (no numpy)
//!   C. one Python call per tick, numpy views over the same memory (zero copy)
//! plus the cost of attaching to the interpreter (taking the GIL).
//!
//! Run with: cargo run --release -p spike_pyo3_cost

use std::ffi::{CString, c_char};
use std::hint::black_box;
use std::time::Instant;

use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyDict;

const DT: f32 = 1.0 / 60.0;
const TICKS: usize = 50;

const SCRIPT: &str = r#"
DT = 1.0 / 60.0

def per_entity(x, y, vx, vy):
    return x + vx * DT, y + vy * DT

def per_column_loop(x, y, vx, vy):
    for i in range(len(x)):
        x[i] += vx[i] * DT
        y[i] += vy[i] * DT

try:
    import numpy as np
    def per_column_numpy(x, y, vx, vy):
        X = np.frombuffer(x, dtype=np.float32)
        Y = np.frombuffer(y, dtype=np.float32)
        X += np.frombuffer(vx, dtype=np.float32) * np.float32(DT)
        Y += np.frombuffer(vy, dtype=np.float32) * np.float32(DT)
    NUMPY = np.__version__
except ImportError as e:
    per_column_numpy = None
    NUMPY = None
"#;

struct Columns {
    x: Vec<f32>,
    y: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
}

impl Columns {
    fn new(n: usize) -> Self {
        Self {
            x: (0..n).map(|i| i as f32).collect(),
            y: vec![0.0; n],
            vx: vec![1.0; n],
            vy: vec![-2.0; n],
        }
    }
}

/// A writable memoryview over a Rust f32 slice; no copy. The view must not
/// outlive the slice: Python code here only uses it during the call.
fn view<'py>(py: Python<'py>, data: &mut [f32]) -> Bound<'py, PyAny> {
    unsafe {
        let ptr = ffi::PyMemoryView_FromMemory(
            data.as_mut_ptr() as *mut c_char,
            (data.len() * 4) as ffi::Py_ssize_t,
            ffi::PyBUF_WRITE,
        );
        Bound::from_owned_ptr(py, ptr)
    }
}

fn per_tick_ms(mut f: impl FnMut()) -> f64 {
    f(); // warm up
    let t = Instant::now();
    for _ in 0..TICKS {
        f();
    }
    t.elapsed().as_secs_f64() * 1000.0 / TICKS as f64
}

fn main() {
    sage_script::init().expect("python");
    let g: Py<PyDict> = Python::attach(|py| {
        let g = PyDict::new(py);
        py.run(&CString::new(SCRIPT).unwrap(), Some(&g), None)
            .unwrap();
        g.unbind()
    });

    Python::attach(|py| {
        let sys = py.import("sys").unwrap();
        let version: String = sys.getattr("version").unwrap().extract().unwrap();
        let numpy: Option<String> = g
            .bind(py)
            .get_item("NUMPY")
            .unwrap()
            .unwrap()
            .extract()
            .unwrap();
        println!("python {}", version.split_whitespace().next().unwrap());
        println!("numpy  {}", numpy.as_deref().unwrap_or("not importable"));
        println!(
            "hash_randomization={}",
            sys.getattr("flags")
                .unwrap()
                .getattr("hash_randomization")
                .unwrap()
        );
    });

    // Cost of attaching to the interpreter (GIL acquire + release).
    let n = 100_000;
    let t = Instant::now();
    for _ in 0..n {
        Python::attach(|py| {
            black_box(py);
        });
    }
    println!(
        "attach+detach: {:.0} ns",
        t.elapsed().as_secs_f64() * 1e9 / n as f64
    );

    println!(
        "\n{:>6}  {:>14}  {:>14}  {:>14}",
        "N", "A per-entity", "B column loop", "C numpy view"
    );
    for n in [1_000usize, 10_000] {
        let mut c = Columns::new(n);
        let a = Python::attach(|py| {
            let f = g.bind(py).get_item("per_entity").unwrap().unwrap();
            per_tick_ms(|| {
                for i in 0..n {
                    let r = f.call1((c.x[i], c.y[i], c.vx[i], c.vy[i])).unwrap();
                    let (x, y): (f32, f32) = r.extract().unwrap();
                    c.x[i] = x;
                    c.y[i] = y;
                }
            })
        });
        let b = Python::attach(|py| {
            let f = g.bind(py).get_item("per_column_loop").unwrap().unwrap();
            per_tick_ms(|| {
                let args = (
                    view(py, &mut c.x).call_method1("cast", ("f",)).unwrap(),
                    view(py, &mut c.y).call_method1("cast", ("f",)).unwrap(),
                    view(py, &mut c.vx).call_method1("cast", ("f",)).unwrap(),
                    view(py, &mut c.vy).call_method1("cast", ("f",)).unwrap(),
                );
                f.call1(args).unwrap();
            })
        });
        let cc = Python::attach(|py| {
            let f = g.bind(py).get_item("per_column_numpy").unwrap().unwrap();
            if f.is_none() {
                return None;
            }
            Some(per_tick_ms(|| {
                let args = (
                    view(py, &mut c.x),
                    view(py, &mut c.y),
                    view(py, &mut c.vx),
                    view(py, &mut c.vy),
                );
                f.call1(args).unwrap();
            }))
        });
        // Every method integrated the same columns; check the numbers moved
        // as expected (2 + 2*TICKS + 1 ticks if numpy ran, else 2*TICKS + 2).
        let ticks = (TICKS + 1) * if cc.is_some() { 3 } else { 2 };
        let expect = (n - 1) as f32 + ticks as f32 * DT;
        assert!(
            (c.x[n - 1] - expect).abs() < 1e-2,
            "x={} expected {expect}",
            c.x[n - 1]
        );
        println!(
            "{n:>6}  {a:>11.3} ms  {b:>11.3} ms  {:>14}",
            cc.map_or("n/a".to_string(), |v| format!("{v:.3} ms"))
        );
    }
    println!("\n(per tick; a 60 Hz tick has 16.7 ms in total)");
}
