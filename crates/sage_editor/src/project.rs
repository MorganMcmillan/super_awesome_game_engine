//! A project open in the editor: Python sources, a live world, a document.
//!
//! - **Edit mode** keeps the live world equal to the document's scene
//!   (`sync`). Nothing ticks.
//! - **Play in editor** (`play`) snapshots the world, runs ticks, and `stop`
//!   restores the snapshot, so playing never changes what is authored
//!   (design doc 10.7 item 4).
//! - **Live reload** (`set_source`, `reload`) builds a fresh world from the
//!   new sources and carries the scene over. A script with an error leaves
//!   the running world alone and reports the error.

use crate::document::Document;
use crate::history::{SourceChange, Step};
use flecs_ecs::prelude::*;
use sage_core::TICK_DT;
use sage_ecs_py::{Ecs, Policy, Snapshot};
use sage_scene::Scene;
use sage_script::Script;
use std::path::Path;

/// The authored scene of a project directory.
pub const SCENE_FILE: &str = "scene.sage";
/// The editor's settings for a project (one `key value` per line).
pub const SETTINGS_FILE: &str = "settings.sage";

/// Fields drop in order: the binding, the scripts, then the world.
pub struct Runtime {
    pub ecs: Ecs,
    scripts: Vec<Script>,
    pub world: World,
}

/// How a host prepares a new world before the scripts load (native
/// components, flecs modules, the UI kit).
pub type Prepare = Box<dyn Fn(&World, &Ecs) -> Result<(), String>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Edit,
    Play,
}

pub struct Project {
    pub name: String,
    /// Editor settings kept with the project (`settings.sage`: one
    /// `key value` per line), such as "suggest_by_type".
    pub settings: std::collections::BTreeMap<String, String>,
    pub sources: Vec<(String, String)>,
    pub doc: Document,
    pub mode: Mode,
    pub tick: u64,
    /// The last script error (a failed reload, or a fault while playing).
    pub error: Option<String>,
    /// What the last scene load repaired or migrated.
    pub warnings: Vec<String>,
    /// Sources changed since the last successful reload.
    pub stale: bool,
    /// Run scripts instrumented, so blocks can glow while they execute.
    pub instrument: bool,
    /// The scripts as last opened or saved, to know when there is something
    /// to save.
    saved_sources: Vec<(String, String)>,
    runtime: Runtime,
    backup: Option<Snapshot>,
    prepare: Prepare,
    seed: u64,
    module: String,
}

fn build(
    sources: &[(String, String)],
    prepare: &Prepare,
    seed: u64,
    module: &str,
    instrument: bool,
) -> Result<Runtime, String> {
    let world = World::new();
    let ecs = Ecs::new(&world, seed, module).map_err(|e| e.0)?;
    prepare(&world, &ecs)?;
    let mut scripts = vec![];
    for (name, src) in sources {
        let src = if instrument {
            crate::blocks::instrument(src).map_err(|e| format!("{name}: {e}"))?
        } else {
            src.clone()
        };
        scripts.push(
            ecs.load(name, &src)
                .map_err(|e| format!("{name}: {}", e.0))?,
        );
    }
    Ok(Runtime {
        ecs,
        scripts,
        world,
    })
}

impl Project {
    /// Opens a project. `scene` is the authored scene text; without it the
    /// scene is whatever the scripts create.
    pub fn open(
        name: &str,
        sources: Vec<(String, String)>,
        scene: Option<&str>,
        prepare: Prepare,
    ) -> Result<Project, String> {
        let (seed, module) = (1, "game");
        let runtime = build(&sources, &prepare, seed, module, false)?;
        let schemas = runtime.ecs.schemas();
        let mut warnings = vec![];
        let scene = match scene {
            Some(text) => {
                warnings = runtime
                    .ecs
                    .load_scene(text, Policy::Lenient)
                    .map_err(|e| format!("scene: {e}"))?;
                Scene::parse(&runtime.ecs.save_scene()).map_err(|e| e.to_string())?
            }
            None => Scene::parse(&runtime.ecs.save_scene()).map_err(|e| e.to_string())?,
        };
        let mut doc = Document::new(scene, schemas);
        doc.unsynced = false;
        Ok(Project {
            name: name.to_string(),
            settings: Default::default(),
            saved_sources: sources.clone(),
            sources,
            doc,
            mode: Mode::Edit,
            tick: 0,
            error: None,
            warnings,
            stale: false,
            instrument: false,
            runtime,
            backup: None,
            prepare,
            seed,
            module: module.to_string(),
        })
    }

    /// Opens a project directory: every `*.py` file in name order, the scene
    /// in `scene.sage` if there is one, and the settings in `settings.sage`.
    pub fn open_dir(dir: &Path, prepare: Prepare) -> Result<Project, String> {
        let read = |p: &Path| std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()));
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "py"))
            .collect();
        files.sort();
        if files.is_empty() {
            return Err(format!("{} has no .py files", dir.display()));
        }
        let mut sources = vec![];
        for p in files {
            let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            sources.push((name, read(&p)?));
        }
        // A project without a scene or settings file is a new one.
        let scene = std::fs::read_to_string(dir.join(SCENE_FILE)).ok();
        let name = dir
            .file_name()
            .map_or("project".into(), |n| n.to_string_lossy().to_string());
        let mut project = Project::open(&name, sources, scene.as_deref(), prepare)?;
        if let Ok(text) = std::fs::read_to_string(dir.join(SETTINGS_FILE)) {
            project.load_settings(&text);
        }
        Ok(project)
    }

    /// Writes the scripts, the scene and (when any was set) the settings
    /// into `dir`, and marks the document saved.
    pub fn save_dir(&mut self, dir: &Path) -> Result<(), String> {
        let write = |name: &str, text: &str| {
            let path = dir.join(name);
            std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
        };
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for (name, text) in &self.sources {
            write(&format!("{name}.py"), text)?;
        }
        write(SCENE_FILE, &self.doc.scene.write())?;
        if !self.settings.is_empty() {
            write(SETTINGS_FILE, &self.settings_text())?;
        }
        self.doc.mark_saved();
        self.saved_sources = self.sources.clone();
        Ok(())
    }

    pub fn ecs(&self) -> &Ecs {
        &self.runtime.ecs
    }

    /// Reads `key value` lines (blank lines and `#` comments are skipped).
    pub fn load_settings(&mut self, text: &str) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once(char::is_whitespace) {
                self.settings.insert(k.to_string(), v.trim().to_string());
            }
        }
    }

    /// The settings as `key value` lines, sorted by key.
    pub fn settings_text(&self) -> String {
        self.settings
            .iter()
            .map(|(k, v)| format!("{k} {v}\n"))
            .collect()
    }

    /// Loads the document's scene into the live world when it changed. Does
    /// nothing while playing.
    pub fn sync(&mut self) -> Result<(), String> {
        if self.mode != Mode::Edit || !self.doc.unsynced {
            return Ok(());
        }
        self.doc.unsynced = false;
        let text = self.doc.scene.write();
        self.warnings = self
            .runtime
            .ecs
            .load_scene(&text, Policy::Lenient)
            .map_err(|e| format!("scene: {e}"))?;
        Ok(())
    }

    pub fn source(&self, name: &str) -> Option<&str> {
        self.sources
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s.as_str())
    }

    /// Changes a script's text as one undoable step. It takes effect at the
    /// next `reload`.
    pub fn set_source(&mut self, name: &str, text: &str) -> Result<(), String> {
        self.edit_source(name, text, &format!("Edit {name}.py"))
    }

    /// [`Project::set_source`] with the step's name ("Delete stack move").
    pub fn edit_source(&mut self, name: &str, text: &str, label: &str) -> Result<(), String> {
        let before = self.put_source(name, text)?;
        let change = SourceChange {
            name: name.to_string(),
            before,
            after: text.to_string(),
        };
        self.doc.history.record_source(label, change);
        Ok(())
    }

    /// Gives a script new text without making history. Returns the old text.
    fn put_source(&mut self, name: &str, text: &str) -> Result<String, String> {
        let slot = self
            .sources
            .iter_mut()
            .find(|(n, _)| n == name)
            .ok_or_else(|| format!("no script {name}"))?;
        if slot.1 != text {
            self.stale = true;
            self.doc.revision += 1;
        }
        Ok(std::mem::replace(&mut slot.1, text.to_string()))
    }

    /// Takes back the last step, whether it changed the scene, a script or
    /// both. A script that changed is reloaded at the next `reload`.
    pub fn undo(&mut self) -> Option<Step> {
        let step = self.doc.undo()?;
        self.finish(step)
    }

    pub fn redo(&mut self) -> Option<Step> {
        let step = self.doc.redo()?;
        self.finish(step)
    }

    /// The scripts' half of an undo or redo.
    fn finish(&mut self, step: Step) -> Option<Step> {
        for (name, text) in &step.sources {
            // The script was there when the step was recorded; scripts are
            // never removed from an open project.
            let _ = self.put_source(name, text);
        }
        Some(step)
    }

    /// There is something to save: the scene or a script differs from what
    /// was last opened or saved.
    pub fn dirty(&self) -> bool {
        self.doc.dirty() || self.sources != self.saved_sources
    }

    /// Live reload: runs the sources in a new world and carries the scene
    /// over. On a script error the old world stays and `error` says why.
    pub fn reload(&mut self) -> Result<(), String> {
        if self.mode == Mode::Play {
            self.stop()?;
        }
        self.sync()?;
        let fresh = match build(
            &self.sources,
            &self.prepare,
            self.seed,
            &self.module,
            self.instrument,
        ) {
            Ok(r) => r,
            Err(e) => {
                self.error = Some(e.clone());
                return Err(e);
            }
        };
        let text = self.doc.scene.write();
        match fresh.ecs.load_scene(&text, Policy::Lenient) {
            Ok(w) => self.warnings = w,
            Err(e) => {
                let e = format!("scene: {e}");
                self.error = Some(e.clone());
                return Err(e);
            }
        }
        // Components may have gained or lost fields: the scene follows them.
        let mut scene = self.doc.scene.clone();
        let schemas = fresh.ecs.schemas();
        scene
            .conform(&schemas, Policy::Lenient)
            .map_err(|e| format!("scene: {e}"))?;
        // A scene difference recorded under other fields cannot be applied
        // to this scene: when the components changed shape, the scene edits
        // before now become permanent. Script edits stay undoable.
        if schemas != self.doc.schemas && self.doc.history.drop_scene_steps() > 0 {
            self.warnings
                .push("Components changed: earlier scene edits can no longer be undone".into());
        }
        self.runtime = fresh;
        self.doc.schemas = schemas;
        self.doc.scene = scene;
        self.doc.unsynced = false;
        self.doc.select(self.doc.selection);
        self.error = None;
        self.stale = false;
        Ok(())
    }

    /// Turns instrumentation on or off and reloads (blocks glow while
    /// the game runs).
    pub fn set_instrument(&mut self, on: bool) -> Result<(), String> {
        if self.instrument != on {
            self.instrument = on;
            self.reload()?;
        }
        Ok(())
    }

    /// Starts playing: snapshots the world, then `step` runs ticks.
    pub fn play(&mut self) -> Result<(), String> {
        if self.mode == Mode::Play {
            return Ok(());
        }
        if self.stale {
            self.reload()?;
        }
        self.sync()?;
        self.runtime.ecs.clear_fault();
        self.backup = Some(self.runtime.ecs.snapshot());
        self.mode = Mode::Play;
        self.tick = 0;
        self.error = None;
        Ok(())
    }

    /// Runs `n` fixed ticks of the game. A fault in a Python system stops
    /// the play session and reports it.
    pub fn step(&mut self, n: u32) -> Result<(), String> {
        if self.mode != Mode::Play {
            return Err("not playing".into());
        }
        for _ in 0..n {
            self.runtime.world.progress_time(TICK_DT);
            self.tick += 1;
            if let Some(f) = self.runtime.ecs.fault() {
                self.error = Some(f.clone());
                self.stop()?;
                return Err(f);
            }
        }
        Ok(())
    }

    /// Stops playing and puts the world back as it was.
    pub fn stop(&mut self) -> Result<(), String> {
        if self.mode != Mode::Play {
            return Ok(());
        }
        self.mode = Mode::Edit;
        let snap = self.backup.take().expect("play takes a snapshot");
        self.runtime.ecs.clear_fault();
        self.runtime.ecs.restore(&snap)?;
        self.tick = 0;
        Ok(())
    }

    /// Runs `source` (a function definition, say) in the first script's names
    /// and then calls `func` with no arguments: a stray stack run by hand
    /// (design doc 4.1), or a stack cut short at a block ("run up to here").
    /// In edit mode what it did to the world becomes one undoable edit.
    pub fn run_source(&mut self, source: &str, func: &str) -> Result<(), String> {
        use pyo3::prelude::*;
        use pyo3::types::PyDictMethods;
        let first = self.runtime.scripts.first().ok_or("no script is loaded")?;
        let ecs_ptr = &self.runtime.ecs;
        ecs_ptr.clear_fault();
        Python::attach(|py| -> PyResult<()> {
            let ns = first.globals(py).copy()?;
            let code = std::ffi::CString::new(source)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
            py.run(&code, Some(&ns), None)?;
            let f = ns
                .get_item(func)?
                .ok_or_else(|| pyo3::exceptions::PyNameError::new_err(func.to_string()))?;
            f.call0()?;
            Ok(())
        })
        .map_err(|e| Python::attach(|py| sage_script::describe(py, &e).0))?;
        if self.mode == Mode::Edit {
            let live = self.live_scene();
            self.doc.replace_scene(&format!("Run {func}"), live);
            self.doc.unsynced = false;
        }
        Ok(())
    }

    /// Flecs modules of the live world and whether each is enabled.
    pub fn modules(&self) -> Vec<(String, bool)> {
        self.runtime.ecs.modules()
    }

    /// Enables or disables a flecs module the scripts declared (by the variable
    /// the script bound it to). Pausing a feature (design doc 5.5).
    pub fn set_module_enabled(&mut self, name: &str, on: bool) -> Result<(), String> {
        use pyo3::prelude::*;
        use pyo3::types::PyDictMethods;
        let first = self.runtime.scripts.first().ok_or("no script is loaded")?;
        Python::attach(|py| -> PyResult<()> {
            let g = first.globals(py);
            // The module is the script's variable for it (`combat = world.module(..)`).
            let m = g
                .get_item(name)?
                .ok_or_else(|| pyo3::exceptions::PyNameError::new_err(name.to_string()))?;
            m.call_method0(if on { "enable" } else { "disable" })?;
            Ok(())
        })
        .map_err(|e| Python::attach(|py| sage_script::describe(py, &e).0))
    }

    /// The scene the live world holds now (while playing, what the game
    /// has done to it).
    pub fn live_scene(&self) -> Scene {
        Scene::parse(&self.runtime.ecs.save_scene()).expect("the world writes valid scenes")
    }

    /// The scene the viewport shows: the live world while playing, the
    /// document otherwise.
    pub fn shown_scene(&self) -> Scene {
        match self.mode {
            Mode::Play => self.live_scene(),
            Mode::Edit => self.doc.scene.clone(),
        }
    }
}
