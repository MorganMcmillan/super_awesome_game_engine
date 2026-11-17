//! The `editor` object the editor's UI scripts talk to.
//!
//! This is the adapter between the screen (Python, `scripts/`) and the
//! [`Session`]: each method is one question about the session or one edit of
//! its document. Document edits are pure data operations, so scripts may
//! call them from inside UI observers. Anything that runs the *game's*
//! Python (play, stop, reload) is only requested here and done by
//! [`Session::tick`] after the UI tick.

use crate::document::{EntityView, short};
use crate::history::Step;
use crate::project::Mode;
use crate::session::{Request, Session};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

/// A session shared between the host and the object scripts hold.
pub type Shared = Rc<RefCell<Session>>;

thread_local! {
    static SESSIONS: RefCell<BTreeMap<u64, Shared>> = const { RefCell::new(BTreeMap::new()) };
}

/// Python objects hold only an id, never an `Rc` (they are garbage Python
/// may free on any thread); lookups succeed on the editor's own thread.
#[pyclass(name = "Editor")]
pub struct EditorHost {
    id: u64,
}

/// `(field, kind, text)`.
type Field = (String, String, String);
/// `(sid, name, [(path, name, [field])], [tag])`.
type Inspected = (u64, String, Vec<(String, String, Vec<Field>)>, Vec<String>);

impl EditorHost {
    /// Registers `session` and returns the object scripts get. Call
    /// [`unregister`] with its id when the editor closes.
    pub fn register(session: Shared) -> EditorHost {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        SESSIONS.with(|h| h.borrow_mut().insert(id, session));
        EditorHost { id }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    fn session(&self) -> Shared {
        SESSIONS
            .with(|h| h.borrow().get(&self.id).cloned())
            .unwrap_or_else(|| {
                panic!("the editor object is used off its own thread or after it was closed")
            })
    }

    fn read<R>(&self, f: impl FnOnce(&Session) -> R) -> R {
        f(&self.session().borrow())
    }

    fn write<R>(&self, f: impl FnOnce(&mut Session) -> R) -> R {
        f(&mut self.session().borrow_mut())
    }

    /// An edit of the document: `None` when it was made, else why not.
    fn edit(&self, f: impl FnOnce(&mut Session) -> Result<(), String>) -> Option<String> {
        self.write(f).err()
    }
}

pub fn unregister(id: u64) {
    SESSIONS.with(|h| h.borrow_mut().remove(&id));
}

/// An entity's view in the shape scripts take it.
fn inspected(v: EntityView) -> Inspected {
    let components = v.components.into_iter().map(|c| {
        let fields = c.fields.into_iter();
        let fields = fields.map(|f| (f.name, f.kind.name().to_string(), f.text));
        (c.path, c.name, fields.collect())
    });
    (v.sid, v.name.unwrap_or_default(), components.collect(), v.tags)
}

/// What scripts are told about an undo or redo.
fn told(step: Step) -> (String, Option<String>) {
    let script = step.sources.into_iter().next().map(|(name, _)| name);
    (step.label, script)
}

#[pymethods]
impl EditorHost {
    /// Changes whenever a view needs to be rebuilt.
    fn revision(&self) -> u64 {
        self.read(Session::revision)
    }

    // ---- the document ------------------------------------------------------------------

    fn outliner(&self) -> Vec<(u64, usize, String)> {
        self.read(|s| {
            let rows = s.project.doc.outliner();
            rows.into_iter().map(|r| (r.sid, r.depth, r.label)).collect()
        })
    }

    fn selection(&self) -> Option<u64> {
        self.read(|s| s.project.doc.selection)
    }

    #[pyo3(signature = (sid))]
    fn select(&self, sid: Option<u64>) {
        self.write(|s| s.project.doc.select(sid));
    }

    /// The selected entity as authored.
    fn inspect(&self) -> Option<Inspected> {
        self.read(|s| {
            let sid = s.project.doc.selection?;
            s.project.doc.inspect(sid).map(inspected)
        })
    }

    /// The selected entity as the running game has it now (`None` when the
    /// game is not running, or it has destroyed the entity).
    fn inspect_live(&self) -> Option<Inspected> {
        self.read(|s| {
            let sid = s.project.doc.selection?;
            crate::document::inspect(s.live()?, sid).map(inspected)
        })
    }

    /// `(sid, depth, label)` of the entities the running game made. They are
    /// not authored: they are gone when the game stops.
    fn runtime_only(&self) -> Vec<(u64, usize, String)> {
        self.read(|s| {
            let rows = s.runtime_only().into_iter();
            rows.map(|r| (r.sid, r.depth, r.label)).collect()
        })
    }

    /// Keeps one runtime value: sets the authored field to what the running
    /// game has now (an undoable edit).
    fn keep(&self, sid: u64, component: &str, field: &str) -> Option<String> {
        self.edit(|s| s.keep(sid, component, field))
    }

    /// The scene viewport's grid: dragged entities land on multiples of it
    /// (0 is off).
    fn snap(&self) -> f32 {
        self.read(|s| s.scene_view.snap)
    }

    fn set_snap(&self, units: f32) {
        self.write(|s| {
            s.scene_view.snap = units.max(0.0);
            s.touch();
        });
    }

    /// `(path, short name)` of what the selected entity could still be given.
    fn addable(&self) -> Vec<(String, String)> {
        self.read(|s| {
            let Some(sid) = s.project.doc.selection else {
                return vec![];
            };
            let paths = s.project.doc.addable(sid).into_iter();
            paths.map(|c| (c.clone(), short(&c).to_string())).collect()
        })
    }

    fn set_field(&self, sid: u64, component: &str, field: &str, text: &str) -> Option<String> {
        self.edit(|s| s.project.doc.set_field(sid, component, field, text))
    }

    fn end_gesture(&self) {
        self.write(|s| s.project.doc.end_gesture());
    }

    fn add_component(&self, sid: u64, path: &str) -> Option<String> {
        self.edit(|s| s.project.doc.add_component(sid, path))
    }

    fn remove_component(&self, sid: u64, path: &str) -> Option<String> {
        self.edit(|s| s.project.doc.remove_component(sid, path))
    }

    fn rename(&self, sid: u64, name: &str) -> Option<String> {
        self.edit(|s| s.project.doc.rename(sid, name))
    }

    #[pyo3(signature = (name, parent=None))]
    fn spawn(&self, name: &str, parent: Option<u64>) -> PyResult<u64> {
        self.write(|s| s.project.doc.spawn(name, parent))
            .map_err(PyValueError::new_err)
    }

    fn despawn(&self, sid: u64) -> Option<String> {
        self.edit(|s| s.project.doc.despawn(sid))
    }

    fn duplicate(&self, sid: u64) -> PyResult<u64> {
        self.write(|s| s.project.doc.duplicate(sid))
            .map_err(PyValueError::new_err)
    }

    /// Takes back the last step, scene or script: `(label, script)`, where
    /// `script` names the script it changed (`None` for a scene step).
    fn undo(&self) -> Option<(String, Option<String>)> {
        self.write(|s| s.project.undo()).map(told)
    }

    fn redo(&self) -> Option<(String, Option<String>)> {
        self.write(|s| s.project.redo()).map(told)
    }

    fn undo_label(&self) -> Option<String> {
        self.read(|s| s.project.doc.history.undo_label().map(str::to_string))
    }

    fn redo_label(&self) -> Option<String> {
        self.read(|s| s.project.doc.history.redo_label().map(str::to_string))
    }

    /// There is something to save: the scene or a script changed.
    fn dirty(&self) -> bool {
        self.read(|s| s.project.dirty())
    }

    /// How many entities are instances of the prefab called `name`.
    fn instances_of(&self, name: &str) -> usize {
        self.read(|s| s.project.doc.instances_of(name))
    }

    /// The reflected components as `{path: [(field, kind)]}`, for the block
    /// palette.
    fn schemas(&self) -> BTreeMap<String, Vec<(String, String)>> {
        self.read(|s| {
            let schemas = s.project.doc.schemas.iter();
            schemas
                .map(|(path, schema)| {
                    let fields = schema.fields.iter();
                    let fields = fields.map(|(n, k)| (n.clone(), k.name().to_string()));
                    (path.clone(), fields.collect())
                })
                .collect()
        })
    }

    // ---- the scripts -------------------------------------------------------------------

    fn sources(&self) -> Vec<String> {
        self.read(|s| s.project.sources.iter().map(|(n, _)| n.clone()).collect())
    }

    fn source(&self, name: &str) -> String {
        self.read(|s| s.project.source(name).unwrap_or_default().to_string())
    }

    /// Changes a script's text as one undoable step called `label`.
    #[pyo3(signature = (name, text, label=None))]
    fn set_source(&self, name: &str, text: &str, label: Option<&str>) -> Option<String> {
        self.edit(|s| {
            s.touch();
            match label {
                Some(label) => s.project.edit_source(name, text, label),
                None => s.project.set_source(name, text),
            }
        })
    }

    /// A script changed and the game has not been reloaded with it yet.
    fn stale(&self) -> bool {
        self.read(|s| s.project.stale)
    }

    /// Runs `source` then calls `func` in the game (a stray stack, or one
    /// cut short at a block). Returns the last line of an error.
    fn run_source(&self, source: &str, func: &str) -> Option<String> {
        self.write(|s| {
            s.touch();
            let failed = s.project.run_source(source, func).err()?;
            Some(failed.trim().lines().last().unwrap_or("").to_string())
        })
    }

    // ---- the game ----------------------------------------------------------------------

    /// Asks the host to `play`, `stop`, `pause`, `reload`, `step` or `save`
    /// after this tick.
    fn request(&self, what: &str) -> PyResult<()> {
        let request = Request::parse(what)
            .ok_or_else(|| PyValueError::new_err(format!("unknown request {what:?}")))?;
        self.write(|s| s.request(request));
        Ok(())
    }

    fn playing(&self) -> bool {
        self.read(|s| s.project.mode == Mode::Play)
    }

    fn paused(&self) -> bool {
        self.read(|s| s.paused)
    }

    fn tick(&self) -> u64 {
        self.read(|s| s.project.tick)
    }

    /// The last script error (a failed reload, or a fault while playing).
    fn error(&self) -> Option<String> {
        self.read(|s| s.project.error.clone())
    }

    fn warnings(&self) -> Vec<String> {
        self.read(|s| s.project.warnings.clone())
    }

    /// What the last request did, for the status line.
    fn log(&self) -> String {
        self.read(|s| s.log.clone())
    }

    /// The live world's flecs modules and whether each is on.
    fn modules(&self) -> Vec<(String, bool)> {
        self.read(|s| s.project.modules())
    }

    fn set_module(&self, name: &str, on: bool) -> Option<String> {
        self.edit(|s| {
            s.touch();
            s.project.set_module_enabled(name, on)
        })
    }

    // ---- settings ----------------------------------------------------------------------

    /// A project setting, or `default` when it was never set.
    fn setting(&self, key: &str, default: &str) -> String {
        self.read(|s| {
            let set = s.project.settings.get(key);
            set.map_or(default, String::as_str).to_string()
        })
    }

    fn set_setting(&self, key: &str, value: &str) {
        self.write(|s| {
            let settings = &mut s.project.settings;
            settings.insert(key.to_string(), value.to_string());
            s.touch();
        });
    }
}
