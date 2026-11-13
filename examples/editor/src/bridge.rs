//! The editor object the UI scripts talk to (`editor` in `scripts/editor.py`).
//!
//! Document edits are pure data operations, so scripts may call them from
//! inside UI observers. Anything that runs the *game's* Python (play, stop,
//! reload) is only requested here and done by the host after the UI tick.

use pyo3::prelude::*;
use sage_editor::{Mode, Project};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub struct Shared {
    pub project: RefCell<Project>,
    pub requests: RefCell<Vec<String>>,
    /// Bumped by the host when something views show changed without the
    /// document changing (play started, an error appeared).
    pub extra: std::cell::Cell<u64>,
    pub log: RefCell<String>,
    pub paused: std::cell::Cell<bool>,
}

thread_local! {
    static HOSTS: RefCell<BTreeMap<u64, Rc<Shared>>> = const { RefCell::new(BTreeMap::new()) };
}

/// Python objects hold only an id, never an `Rc` (they are garbage Python
/// may free on any thread); lookups succeed on the editor's own thread.
#[pyclass(name = "Editor")]
pub struct EditorHost {
    id: u64,
}

impl EditorHost {
    /// Registers `shared` and returns the object scripts get.
    pub fn register(shared: Rc<Shared>) -> EditorHost {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        HOSTS.with(|h| h.borrow_mut().insert(id, shared));
        EditorHost { id }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    fn shared(&self) -> Rc<Shared> {
        HOSTS
            .with(|h| h.borrow().get(&self.id).cloned())
            .unwrap_or_else(|| {
                panic!("the editor object is used off its own thread or after it was closed")
            })
    }
}

pub fn unregister(id: u64) {
    HOSTS.with(|h| h.borrow_mut().remove(&id));
}

fn err(r: Result<(), String>) -> Option<String> {
    r.err()
}

#[pymethods]
impl EditorHost {
    /// Changes whenever a view needs to be rebuilt.
    fn revision(&self) -> u64 {
        let sh = self.shared();
        sh.project.borrow().doc.revision * 1000 + sh.extra.get()
    }

    fn outliner(&self) -> Vec<(u64, usize, String)> {
        let sh = self.shared();
        let p = sh.project.borrow();
        p.doc
            .outliner()
            .into_iter()
            .map(|r| (r.sid, r.depth, r.label))
            .collect()
    }

    fn selection(&self) -> Option<u64> {
        let sh = self.shared();
        sh.project.borrow().doc.selection
    }

    #[pyo3(signature = (sid))]
    fn select(&self, sid: Option<u64>) {
        let sh = self.shared();
        sh.project.borrow_mut().doc.select(sid);
    }

    /// `(sid, name, [(path, name, [(field, kind, text)])], [tag])` of the
    /// selected entity.
    #[allow(clippy::type_complexity)]
    fn inspect(
        &self,
    ) -> Option<(
        u64,
        String,
        Vec<(String, String, Vec<(String, String, String)>)>,
        Vec<String>,
    )> {
        let sh = self.shared();
        let p = sh.project.borrow();
        let sid = p.doc.selection?;
        let v = p.doc.inspect(sid)?;
        Some((
            sid,
            v.name.unwrap_or_default(),
            v.components
                .into_iter()
                .map(|c| {
                    (
                        c.path,
                        c.name,
                        c.fields
                            .into_iter()
                            .map(|f| (f.name, f.kind.name().to_string(), f.text))
                            .collect(),
                    )
                })
                .collect(),
            v.tags,
        ))
    }

    fn addable(&self) -> Vec<(String, String)> {
        let sh = self.shared();
        let p = sh.project.borrow();
        let Some(sid) = p.doc.selection else {
            return vec![];
        };
        p.doc
            .addable(sid)
            .into_iter()
            .map(|c| {
                let n = sage_editor::document::short(&c).to_string();
                (c, n)
            })
            .collect()
    }

    fn set_field(&self, sid: u64, component: &str, field: &str, text: &str) -> Option<String> {
        let sh = self.shared();
        err(sh
            .project
            .borrow_mut()
            .doc
            .set_field(sid, component, field, text))
    }

    fn end_gesture(&self) {
        let sh = self.shared();
        sh.project.borrow_mut().doc.end_gesture();
    }

    fn add_component(&self, sid: u64, path: &str) -> Option<String> {
        let sh = self.shared();
        err(sh.project.borrow_mut().doc.add_component(sid, path))
    }

    fn remove_component(&self, sid: u64, path: &str) -> Option<String> {
        let sh = self.shared();
        err(sh.project.borrow_mut().doc.remove_component(sid, path))
    }

    fn rename(&self, sid: u64, name: &str) -> Option<String> {
        let sh = self.shared();
        err(sh.project.borrow_mut().doc.rename(sid, name))
    }

    #[pyo3(signature = (name, parent=None))]
    fn spawn(&self, name: &str, parent: Option<u64>) -> PyResult<u64> {
        let sh = self.shared();
        sh.project
            .borrow_mut()
            .doc
            .spawn(name, parent)
            .map_err(pyo3::exceptions::PyValueError::new_err)
    }

    fn despawn(&self, sid: u64) -> Option<String> {
        let sh = self.shared();
        err(sh.project.borrow_mut().doc.despawn(sid))
    }

    fn duplicate(&self, sid: u64) -> PyResult<u64> {
        let sh = self.shared();
        sh.project
            .borrow_mut()
            .doc
            .duplicate(sid)
            .map_err(pyo3::exceptions::PyValueError::new_err)
    }

    fn undo(&self) -> Option<String> {
        let sh = self.shared();
        sh.project.borrow_mut().doc.undo()
    }

    fn redo(&self) -> Option<String> {
        let sh = self.shared();
        sh.project.borrow_mut().doc.redo()
    }

    fn undo_label(&self) -> Option<String> {
        let sh = self.shared();
        sh.project
            .borrow()
            .doc
            .history
            .undo_label()
            .map(str::to_string)
    }

    fn redo_label(&self) -> Option<String> {
        let sh = self.shared();
        sh.project
            .borrow()
            .doc
            .history
            .redo_label()
            .map(str::to_string)
    }

    fn dirty(&self) -> bool {
        let sh = self.shared();
        let p = sh.project.borrow();
        p.doc.dirty() || p.stale
    }

    /// Runs `source` then calls `func` in the game (a stray stack, or one
    /// cut short at a block).
    fn run_source(&self, source: &str, func: &str) -> Option<String> {
        let sh = self.shared();
        sh.extra.set(sh.extra.get() + 1);
        let r = sh.project.borrow_mut().run_source(source, func);
        r.err()
            .map(|e| e.trim().lines().last().unwrap_or("").to_string())
    }

    fn modules(&self) -> Vec<(String, bool)> {
        let sh = self.shared();
        sh.project.borrow().modules()
    }

    fn set_module(&self, name: &str, on: bool) -> Option<String> {
        let sh = self.shared();
        sh.extra.set(sh.extra.get() + 1);
        let r = sh.project.borrow_mut().set_module_enabled(name, on);
        r.err()
    }

    /// A project setting, or `default` when it was never set.
    fn setting(&self, key: &str, default: &str) -> String {
        let sh = self.shared();
        let p = sh.project.borrow();
        p.settings
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    fn set_setting(&self, key: &str, value: &str) {
        let sh = self.shared();
        sh.project
            .borrow_mut()
            .settings
            .insert(key.to_string(), value.to_string());
        sh.extra.set(sh.extra.get() + 1);
    }

    fn paused(&self) -> bool {
        let sh = self.shared();
        sh.paused.get()
    }

    /// How many entities are instances of a prefab called `name` (an `IsA`
    /// pair whose target path ends with it).
    fn instances_of(&self, name: &str) -> usize {
        let sh = self.shared();
        let p = sh.project.borrow();
        p.doc
            .scene
            .entities
            .iter()
            .filter(|e| {
                e.pairs.iter().any(|x| {
                    x.rel.ends_with("IsA")
                        && (x.target_path.ends_with(&format!(".{name}"))
                            || x.target_path.ends_with(&format!("::{name}"))
                            || x.target_path == name)
                })
            })
            .count()
    }

    fn playing(&self) -> bool {
        let sh = self.shared();
        sh.project.borrow().mode == Mode::Play
    }

    fn tick(&self) -> u64 {
        let sh = self.shared();
        sh.project.borrow().tick
    }

    fn error(&self) -> Option<String> {
        let sh = self.shared();
        sh.project.borrow().error.clone()
    }

    fn warnings(&self) -> Vec<String> {
        let sh = self.shared();
        sh.project.borrow().warnings.clone()
    }

    fn log(&self) -> String {
        let sh = self.shared();
        sh.log.borrow().clone()
    }

    /// Asks the host to `play`, `stop`, `reload`, `save` or `step`.
    fn request(&self, what: &str) {
        let sh = self.shared();
        sh.requests.borrow_mut().push(what.to_string());
    }

    fn sources(&self) -> Vec<String> {
        let sh = self.shared();
        sh.project
            .borrow()
            .sources
            .iter()
            .map(|(n, _)| n.clone())
            .collect()
    }

    fn source(&self, name: &str) -> String {
        let sh = self.shared();
        sh.project
            .borrow()
            .source(name)
            .unwrap_or_default()
            .to_string()
    }

    fn set_source(&self, name: &str, text: &str) -> Option<String> {
        let sh = self.shared();
        sh.extra.set(sh.extra.get() + 1);
        err(sh.project.borrow_mut().set_source(name, text))
    }

    fn stale(&self) -> bool {
        let sh = self.shared();
        sh.project.borrow().stale
    }

    /// The reflected components as `{path: [(field, kind)]}`, for the block
    /// palette.
    fn schemas(&self) -> BTreeMap<String, Vec<(String, String)>> {
        let sh = self.shared();
        sh.project
            .borrow()
            .doc
            .schemas
            .iter()
            .map(|(p, s)| {
                (
                    p.clone(),
                    s.fields
                        .iter()
                        .map(|(n, k)| (n.clone(), k.name().to_string()))
                        .collect(),
                )
            })
            .collect()
    }
}
