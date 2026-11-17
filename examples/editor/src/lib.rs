//! The editor as an application (design doc 10.6; milestones M3 and M4).
//!
//! Everything that makes it an editor lives in `sage_editor`: the session
//! and its document, the `editor` object scripts talk to, and the screen
//! itself (UI entities written in Python). This crate is what an application
//! adds: a [`UiHost`] to run the screen in, the text face, and the scene
//! viewport's pointer passed from the UI to the session and its picture
//! passed back.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use flecs_ecs::prelude::*;
use pyo3::prelude::*;
use sage_app::{UiHost, UiHosted};
use sage_assets::BitmapFont;
use sage_editor::bridge::{self, EditorHost, Shared};
use sage_editor::{Prepare, Project, ScenePointer, Session, screen};
use sage_ui::ecs::UiViewport;

pub use sage_editor::screen::{CLEAR, SIZE as SCREEN};

/// How a game world is prepared before its scripts load. Hosts with native
/// components register them here; the sample needs nothing.
pub fn no_prepare() -> Prepare {
    Box::new(|_, _| Ok(()))
}

/// Opens the sample project.
pub fn sample_project() -> Result<Project, String> {
    let sources = vec![("game".into(), screen::SAMPLE_GAME.into())];
    Project::open("sample", sources, None, no_prepare())
}

pub struct Editor {
    ui: UiHost,
    pub session: Shared,
    /// The `editor` object's registration, removed when the editor closes.
    host_id: u64,
}

impl std::ops::Deref for Editor {
    type Target = UiHost;
    fn deref(&self) -> &UiHost {
        &self.ui
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        bridge::unregister(self.host_id);
    }
}

impl UiHosted for Editor {
    fn ui(&self) -> &UiHost {
        &self.ui
    }

    /// The session's part of a tick (requests, the playing game, the scene
    /// viewport), then the views that changed are rebuilt.
    fn after_ui(&self) {
        let mut pointer = None;
        self.world.new_query::<&UiViewport>().each(|v| {
            if v.source == screen::SCENE_VIEW {
                pointer = Some(ScenePointer {
                    at: [v.x, v.y],
                    over: v.over != 0,
                    down: v.down & 1 != 0,
                });
            }
        });
        let frame = {
            let mut session = self.session.borrow_mut();
            session.tick(pointer);
            session.frame()
        };
        sage_ui::set_viewport_frame(&self.ecs, screen::SCENE_VIEW, frame);
        self.call(screen::REFRESH);
    }

    /// Views are rebuilt after the tick that handled an event: one more tick
    /// lays the new widgets out.
    fn settle(&self) -> u32 {
        1
    }

    fn font(&self, scale: u32) -> Option<BitmapFont> {
        BitmapFont::from_ttf_proportional(screen::FACE, scale).ok()
    }

    fn clear_color(&self) -> [u8; 4] {
        CLEAR
    }
}

impl Editor {
    /// Opens the editor on `project`, saved to `dir` (the working directory
    /// when it has none).
    pub fn new(mut project: Project, dir: Option<PathBuf>) -> Result<Editor, String> {
        // The editor's face is shaped and proportional. The metrics are per
        // thread, so they are set here, where layout runs.
        let face = BitmapFont::from_ttf_proportional(screen::FACE, 1)?;
        sage_core::host::set_text_metrics(face.shaped.clone().map(|s| s as _));
        sage_editor::blocks::install()?;
        // Blocks glow while the game runs: scripts report their lines.
        project.set_instrument(true)?;
        let session: Shared = Rc::new(RefCell::new(Session::new(project, dir)));
        let host = EditorHost::register(session.clone());
        let host_id = host.id();
        let host = Python::attach(|py| Py::new(py, host).map(Py::into_any))
            .map_err(|e| Python::attach(|py| sage_script::describe(py, &e).0))?;

        let mut ui = UiHost::new("editor", SCREEN)?;
        ui.load("theme", screen::THEME)?;
        ui.load_parts("editor", screen::PARTS, vec![("editor".to_string(), host)])?;
        let ed = Editor {
            ui,
            session,
            host_id,
        };
        ed.step();
        Ok(ed)
    }

    /// Reads or changes the project being edited.
    pub fn project<R>(&self, f: impl FnOnce(&mut Project) -> R) -> R {
        f(&mut self.session.borrow_mut().project)
    }
}
