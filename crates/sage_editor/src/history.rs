//! One transaction log for everything a person edits (design doc 10.7 item
//! 4): the scene and the scripts share a single undo stack, so Undo takes
//! back whatever was done last (decided by Morgan, 2026-11-16).
//!
//! A scene edit is the difference between two scenes: the entities that
//! changed, each with its value before and after. A script edit is the
//! script's text before and after. Undo applies a transaction backwards,
//! redo forwards. Nothing here knows about Flecs or Python: the scene is
//! changed in place, and the script texts to put back are handed to the
//! caller, which owns the scripts.

use sage_scene::{Scene, SceneComponent, SceneEntity};

/// One entity's change. `None` before means it was created; `None` after,
/// deleted.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityChange {
    pub sid: u64,
    pub before: Option<SceneEntity>,
    pub after: Option<SceneEntity>,
}

/// The change between two scenes: entities, and the singletons as a whole
/// (they are few).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneDiff {
    pub entities: Vec<EntityChange>,
    pub singletons: Option<(Vec<SceneComponent>, Vec<SceneComponent>)>,
}

impl SceneDiff {
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty() && self.singletons.is_none()
    }

    /// What changed from `a` to `b`. Both must have their entities sorted by
    /// StableId.
    pub fn between(a: &Scene, b: &Scene) -> SceneDiff {
        let mut out = SceneDiff::default();
        let (mut i, mut j) = (0, 0);
        while i < a.entities.len() || j < b.entities.len() {
            let (ea, eb) = (a.entities.get(i), b.entities.get(j));
            match (ea, eb) {
                (Some(x), Some(y)) if x.sid == y.sid => {
                    if x != y {
                        out.entities.push(EntityChange {
                            sid: x.sid,
                            before: Some(x.clone()),
                            after: Some(y.clone()),
                        });
                    }
                    i += 1;
                    j += 1;
                }
                (Some(x), Some(y)) if x.sid < y.sid => {
                    out.entities.push(EntityChange {
                        sid: x.sid,
                        before: Some(x.clone()),
                        after: None,
                    });
                    i += 1;
                }
                (Some(x), None) => {
                    out.entities.push(EntityChange {
                        sid: x.sid,
                        before: Some(x.clone()),
                        after: None,
                    });
                    i += 1;
                }
                (_, Some(y)) => {
                    out.entities.push(EntityChange {
                        sid: y.sid,
                        before: None,
                        after: Some(y.clone()),
                    });
                    j += 1;
                }
                (None, None) => unreachable!(),
            }
        }
        if a.singletons != b.singletons {
            out.singletons = Some((a.singletons.clone(), b.singletons.clone()));
        }
        out
    }

    /// Applies the change to `scene` (forwards), or undoes it (`forward` is
    /// false).
    pub fn apply(&self, scene: &mut Scene, forward: bool) {
        for c in &self.entities {
            let (from, to) = if forward {
                (&c.before, &c.after)
            } else {
                (&c.after, &c.before)
            };
            let at = scene.entities.binary_search_by_key(&c.sid, |e| e.sid);
            match (from, to, at) {
                (_, Some(e), Ok(k)) => scene.entities[k] = e.clone(),
                (_, Some(e), Err(k)) => scene.entities.insert(k, e.clone()),
                (_, None, Ok(k)) => {
                    scene.entities.remove(k);
                }
                (_, None, Err(_)) => {}
            }
        }
        if let Some((before, after)) = &self.singletons {
            scene.singletons = if forward { after } else { before }.clone();
        }
    }

    /// This change followed by `later`, as one change.
    pub fn then(mut self, later: SceneDiff) -> SceneDiff {
        for c in later.entities {
            match self.entities.iter_mut().find(|x| x.sid == c.sid) {
                Some(x) => x.after = c.after,
                None => self.entities.push(c),
            }
        }
        self.entities.retain(|c| c.before != c.after);
        self.entities.sort_by_key(|c| c.sid);
        self.singletons = match (self.singletons.take(), later.singletons) {
            (Some((b, _)), Some((_, a))) => Some((b, a)),
            (x, None) => x,
            (None, y) => y,
        }
        .filter(|(b, a)| b != a);
        self
    }
}

/// One script's text before and after an edit.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceChange {
    pub name: String,
    pub before: String,
    pub after: String,
}

/// What an undo or redo did, for the caller to finish and to tell the person.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    pub label: String,
    /// The scene was changed (already applied).
    pub scene: bool,
    /// Scripts to give this text: `(name, text)`. Applying them is the
    /// caller's part.
    pub sources: Vec<(String, String)>,
}

/// One undoable step.
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    pub label: String,
    pub diff: SceneDiff,
    pub sources: Vec<SourceChange>,
    /// Edits with the same key merge while they follow each other, so a drag
    /// or typing into one field is a single step.
    pub key: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct History {
    done: Vec<Transaction>,
    undone: Vec<Transaction>,
    /// Set by undo and redo: the next edit never merges into the last one.
    sealed: bool,
    limit: usize,
}

impl History {
    pub fn new() -> History {
        History {
            limit: 1000,
            ..Default::default()
        }
    }

    /// Records a change. An empty one is dropped. Redo is cleared.
    pub fn record(&mut self, label: &str, diff: SceneDiff, key: Option<String>) {
        if diff.is_empty() {
            return;
        }
        self.undone.clear();
        if let (Some(k), Some(last), false) = (&key, self.done.last_mut(), self.sealed)
            && last.key.as_ref() == Some(k)
        {
            let merged = std::mem::take(&mut last.diff).then(diff);
            last.diff = merged;
            last.label = label.to_string();
            if last.diff.is_empty() {
                self.done.pop();
            }
            return;
        }
        self.push(Transaction {
            label: label.to_string(),
            diff,
            sources: vec![],
            key,
        });
    }

    /// Records a change to a script's text. Redo is cleared.
    pub fn record_source(&mut self, label: &str, change: SourceChange) {
        if change.before == change.after {
            return;
        }
        self.undone.clear();
        self.push(Transaction {
            label: label.to_string(),
            diff: SceneDiff::default(),
            sources: vec![change],
            key: None,
        });
    }

    fn push(&mut self, t: Transaction) {
        self.sealed = false;
        self.done.push(t);
        if self.done.len() > self.limit.max(1) {
            self.done.remove(0);
        }
    }

    /// Forgets the scene half of every step (a step that was only a scene
    /// edit goes). Called when the components' fields changed under the
    /// scene: an older scene difference describes entities in a shape that
    /// no longer exists, so it cannot be applied. Returns how many steps
    /// went.
    pub fn drop_scene_steps(&mut self) -> usize {
        let before = self.done.len() + self.undone.len();
        for stack in [&mut self.done, &mut self.undone] {
            for t in stack.iter_mut() {
                t.diff = SceneDiff::default();
            }
            stack.retain(|t| !t.sources.is_empty());
        }
        before - self.done.len() - self.undone.len()
    }

    /// Ends merging: the next edit starts a new step even with the same key
    /// (the pointer was released).
    pub fn seal(&mut self) {
        self.sealed = true;
    }

    /// Takes back the last step: the scene is changed here, the scripts'
    /// texts are returned for the caller to apply.
    pub fn undo(&mut self, scene: &mut Scene) -> Option<Step> {
        let t = self.done.pop()?;
        let step = Self::apply(&t, scene, false);
        self.undone.push(t);
        self.sealed = true;
        Some(step)
    }

    pub fn redo(&mut self, scene: &mut Scene) -> Option<Step> {
        let t = self.undone.pop()?;
        let step = Self::apply(&t, scene, true);
        self.done.push(t);
        self.sealed = true;
        Some(step)
    }

    fn apply(t: &Transaction, scene: &mut Scene, forward: bool) -> Step {
        t.diff.apply(scene, forward);
        let text = |c: &SourceChange| if forward { &c.after } else { &c.before }.clone();
        Step {
            label: t.label.clone(),
            scene: !t.diff.is_empty(),
            sources: t.sources.iter().map(|c| (c.name.clone(), text(c))).collect(),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.done.last().map(|t| t.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.undone.last().map(|t| t.label.as_str())
    }

    /// Labels, oldest first.
    pub fn labels(&self) -> Vec<String> {
        self.done.iter().map(|t| t.label.clone()).collect()
    }

    pub fn clear(&mut self) {
        self.done.clear();
        self.undone.clear();
    }
}
