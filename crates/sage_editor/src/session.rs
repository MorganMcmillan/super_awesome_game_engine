//! An editing session: a project, what the person asked the editor to do,
//! and the part of every tick that is not the UI.
//!
//! The UI's scripts edit the document directly (pure data, safe inside an
//! observer), but anything that runs the *game's* Python (play, stop, step,
//! reload) or touches the disk is only [requested](Session::request) and done
//! by [`Session::tick`], after the UI's tick is over.

use crate::project::{Mode, Project};
use crate::viewport::{ScenePointer, SceneView, scene_frame};
use crate::document::OutlinerRow;
use sage_core::host::Frame;
use sage_scene::Scene;
use std::path::{Path, PathBuf};

/// Something the person asked for that the host does between UI ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Play,
    Stop,
    /// Holds or resumes a playing game (Step still advances it).
    Pause,
    Reload,
    /// One tick of the game, which is then held. Starts playing (paused)
    /// first if it is not.
    Step,
    Save,
}

impl Request {
    pub const ALL: [Request; 6] = [
        Request::Play,
        Request::Stop,
        Request::Pause,
        Request::Reload,
        Request::Step,
        Request::Save,
    ];

    /// The name scripts use for it.
    pub fn name(self) -> &'static str {
        match self {
            Request::Play => "play",
            Request::Stop => "stop",
            Request::Pause => "pause",
            Request::Reload => "reload",
            Request::Step => "step",
            Request::Save => "save",
        }
    }

    pub fn parse(name: &str) -> Option<Request> {
        Request::ALL.into_iter().find(|r| r.name() == name)
    }
}

pub struct Session {
    pub project: Project,
    /// A playing game is held: it does not tick until resumed or stepped.
    pub paused: bool,
    /// What the last request did (or why it failed), for the status line.
    pub log: String,
    pub scene_view: SceneView,
    /// The running world as a scene, while playing: read once a tick, for
    /// the viewport and the inspector's runtime values.
    live: Option<Scene>,
    requests: Vec<Request>,
    /// Bumped when what the views show changed without the document changing
    /// (play started, an error appeared, a script changed).
    shown: u64,
    /// Where the project is saved (the working directory when it has none).
    dir: Option<PathBuf>,
}

impl Session {
    pub fn new(project: Project, dir: Option<PathBuf>) -> Session {
        Session {
            project,
            paused: false,
            log: String::new(),
            scene_view: SceneView::default(),
            live: None,
            requests: vec![],
            shown: 1,
            dir,
        }
    }

    /// Changes whenever a view may need rebuilding.
    pub fn revision(&self) -> u64 {
        self.project.doc.revision + self.shown
    }

    /// Says the views are out of date although the document did not change.
    pub fn touch(&mut self) {
        self.shown += 1;
    }

    pub fn request(&mut self, r: Request) {
        self.requests.push(r);
    }

    /// Writes the scripts, the scene and the settings into the project's
    /// directory. Returns what to tell the person.
    pub fn save(&mut self) -> Result<String, String> {
        let dir = self.dir.clone().unwrap_or_else(|| PathBuf::from("."));
        self.project.save_dir(&dir)?;
        Ok(format!("saved to {}", dir.display()))
    }

    /// Does one request. Returns what to tell the person.
    fn perform(&mut self, r: Request) -> Result<String, String> {
        let p = &mut self.project;
        let done = match r {
            Request::Play => p.play().map(|_| "playing"),
            Request::Stop => {
                self.paused = false;
                p.stop().map(|_| "stopped")
            }
            Request::Pause => {
                self.paused = !self.paused;
                Ok(if self.paused { "paused" } else { "running" })
            }
            Request::Reload => p.reload().map(|_| "reloaded"),
            Request::Step => {
                // Stepping always leaves the game held, one tick on: from
                // edit mode it starts the game paused, so "step" never turns
                // into "play".
                if p.mode == Mode::Edit {
                    p.play()?;
                }
                self.paused = true;
                p.step(1).map(|_| "stepped")
            }
            Request::Save => return self.save(),
        }?;
        Ok(done.to_string())
    }

    /// The host's part of a tick: does what was requested, advances a playing
    /// game one tick (or, while editing, reloads changed scripts and brings
    /// the live world up to the document), then lets the pointer act on the
    /// scene viewport.
    pub fn tick(&mut self, pointer: Option<ScenePointer>) {
        for r in std::mem::take(&mut self.requests) {
            self.log = self.perform(r).unwrap_or_else(|why| why);
            self.touch();
        }
        let p = &mut self.project;
        if p.mode == Mode::Play {
            if !self.paused && p.step(1).is_err() {
                self.log = p.error.clone().unwrap_or_default();
                self.touch();
            }
        } else if p.stale && p.error.is_none() {
            // Live reload: new rules take effect without being asked. A
            // failure is kept in `project.error` for the views to show.
            let _ = p.reload();
            self.touch();
        } else if let Err(e) = p.sync() {
            self.log = e;
        }
        self.live = (self.project.mode == Mode::Play).then(|| self.project.live_scene());
        if let Some(pointer) = pointer
            && self.scene_view.pointer(&mut self.project, pointer)
        {
            self.touch();
        }
    }

    /// The scene on screen: the running world while playing, else what is
    /// authored.
    pub fn shown(&self) -> &Scene {
        self.live.as_ref().unwrap_or(&self.project.doc.scene)
    }

    /// The running world, while playing. Whatever differs from the authored
    /// scene is thrown away when the game stops, unless it is [kept].
    ///
    /// [kept]: Session::keep
    pub fn live(&self) -> Option<&Scene> {
        self.live.as_ref()
    }

    /// What the scene viewport shows now.
    pub fn frame(&self) -> Frame {
        scene_frame(self.shown(), self.project.doc.selection, self.project.tick)
    }

    /// Entities the running game made that the authored scene does not have,
    /// as outliner rows. They are gone when the game stops.
    pub fn runtime_only(&self) -> Vec<OutlinerRow> {
        let Some(live) = &self.live else {
            return vec![];
        };
        let authored = &self.project.doc.scene;
        let mut rows = crate::document::outliner(live);
        rows.retain(|r| authored.entities.binary_search_by_key(&r.sid, |e| e.sid).is_err());
        rows
    }

    /// Keeps one runtime value: the field of the authored entity is set to
    /// what the running game has now, as an ordinary undoable edit. Nothing
    /// else of the play session survives stopping.
    pub fn keep(&mut self, sid: u64, component: &str, field: &str) -> Result<(), String> {
        let live = self.live.as_ref().ok_or("the game is not running")?;
        let value = live
            .entities
            .iter()
            .find(|e| e.sid == sid)
            .and_then(|e| e.components.iter().find(|c| c.path == component))
            .and_then(|c| c.fields.iter().find(|(n, _)| n == field))
            .map(|(_, v)| v.to_string())
            .ok_or_else(|| format!("the running game has no {component}.{field} on #{sid}"))?;
        let doc = &mut self.project.doc;
        doc.end_gesture();
        doc.set_field(sid, component, field, &value)?;
        doc.end_gesture();
        Ok(())
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }
}
