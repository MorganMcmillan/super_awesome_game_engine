//! The scene document: the authored scene, its selection and its history.
//!
//! The inspector is generated from the reflected schema (design doc 4.7: one
//! schema, four consumers). Every edit is a transaction on the scene; the
//! host then loads the scene into the live world.

use crate::history::{History, SceneDiff};
use sage_scene::{Kind, Scene, SceneComponent, SceneEntity, Schemas, Value};

/// Parses what a person typed into a field of `kind`.
pub fn parse_value(kind: Kind, text: &str) -> Result<Value, String> {
    let t = text.trim();
    let bad = || format!("{t:?} is not a {}", kind.name());
    Ok(match kind {
        Kind::Bool => match t.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Value::Bool(true),
            "false" | "0" | "no" | "off" => Value::Bool(false),
            _ => return Err(bad()),
        },
        Kind::U8 => Value::U8(t.parse().map_err(|_| bad())?),
        Kind::I32 => Value::I32(t.parse().map_err(|_| bad())?),
        Kind::U32 => Value::U32(t.parse().map_err(|_| bad())?),
        Kind::I64 => Value::I64(t.parse().map_err(|_| bad())?),
        Kind::U64 => Value::U64(t.parse().map_err(|_| bad())?),
        Kind::Entity => Value::Entity(t.parse().map_err(|_| bad())?),
        Kind::F32 => Value::F32(finite(t.parse::<f32>().map_err(|_| bad())?).ok_or_else(bad)?),
        Kind::F64 => Value::F64(finite(t.parse::<f64>().map_err(|_| bad())?).ok_or_else(bad)?),
    })
}

fn finite<T: num::IsFinite + Copy>(v: T) -> Option<T> {
    v.finite().then_some(v)
}

mod num {
    pub trait IsFinite {
        fn finite(&self) -> bool;
    }
    impl IsFinite for f32 {
        fn finite(&self) -> bool {
            self.is_finite()
        }
    }
    impl IsFinite for f64 {
        fn finite(&self) -> bool {
            self.is_finite()
        }
    }
}

/// The text form of a value (what the inspector shows and accepts back).
pub fn format_value(v: &Value) -> String {
    v.to_string()
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldView {
    pub name: String,
    pub kind: Kind,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComponentView {
    pub path: String,
    /// The last segment of the path.
    pub name: String,
    pub fields: Vec<FieldView>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntityView {
    pub sid: u64,
    pub name: Option<String>,
    pub components: Vec<ComponentView>,
    pub tags: Vec<String>,
    pub pairs: Vec<(String, u64)>,
}

/// A row of the outliner: an entity at a depth, parents first.
#[derive(Clone, Debug, PartialEq)]
pub struct OutlinerRow {
    pub sid: u64,
    pub depth: usize,
    pub label: String,
}

pub fn short(path: &str) -> &str {
    path.rsplit(['.', ':']).next().unwrap_or(path)
}

fn is_child_of(rel: &str) -> bool {
    rel == "ChildOf" || rel.ends_with("::ChildOf") || rel.ends_with(".ChildOf")
}

/// Everything an editor shows of an entity.
pub fn inspect(scene: &Scene, sid: u64) -> Option<EntityView> {
    let e = scene.entities.iter().find(|e| e.sid == sid)?;
    Some(EntityView {
        sid,
        name: e.name.clone(),
        components: e
            .components
            .iter()
            .map(|c| ComponentView {
                path: c.path.clone(),
                name: short(&c.path).to_string(),
                fields: c
                    .fields
                    .iter()
                    .map(|(n, v)| FieldView {
                        name: n.clone(),
                        kind: v.kind(),
                        text: format_value(v),
                    })
                    .collect(),
            })
            .collect(),
        tags: e.tags.clone(),
        pairs: e
            .pairs
            .iter()
            .map(|p| (p.rel.clone(), p.target_sid))
            .collect(),
    })
}

/// The hierarchy as rows: roots first, children under their `ChildOf`
/// target, in StableId order.
pub fn outliner(scene: &Scene) -> Vec<OutlinerRow> {
    let parent_of = |e: &SceneEntity| {
        e.pairs
            .iter()
            .find(|p| is_child_of(&p.rel) && p.target_sid != 0)
            .map(|p| p.target_sid)
    };
    let known = |sid: u64| scene.entities.iter().any(|e| e.sid == sid);
    let mut rows = vec![];
    fn walk(
        scene: &Scene,
        parent: Option<u64>,
        depth: usize,
        rows: &mut Vec<OutlinerRow>,
        parent_of: &dyn Fn(&SceneEntity) -> Option<u64>,
        known: &dyn Fn(u64) -> bool,
    ) {
        for e in &scene.entities {
            let p = parent_of(e).filter(|p| known(*p));
            if p == parent && depth < 64 {
                rows.push(OutlinerRow {
                    sid: e.sid,
                    depth,
                    label: match &e.name {
                        Some(n) => format!("{n} #{}", e.sid),
                        None => format!("#{}", e.sid),
                    },
                });
                walk(scene, Some(e.sid), depth + 1, rows, parent_of, known);
            }
        }
    }
    walk(scene, None, 0, &mut rows, &parent_of, &known);
    rows
}

/// The document being edited.
pub struct Document {
    pub scene: Scene,
    /// The schemas the live world reflects now (the inspector's source).
    pub schemas: Schemas,
    pub history: History,
    pub selection: Option<u64>,
    /// Bumped by every change, so views know when to rebuild.
    pub revision: u64,
    /// Bumped when the scene changed and the live world has not seen it.
    pub unsynced: bool,
    saved: Scene,
}

impl Document {
    pub fn new(scene: Scene, schemas: Schemas) -> Document {
        Document {
            saved: scene.clone(),
            scene,
            schemas,
            history: History::new(),
            selection: None,
            revision: 1,
            unsynced: true,
        }
    }

    /// Replaces the scene without making history (opening a file, a reload).
    pub fn reset(&mut self, scene: Scene, schemas: Schemas) {
        self.saved = scene.clone();
        self.scene = scene;
        self.schemas = schemas;
        self.history.clear();
        if let Some(s) = self.selection
            && !self.scene.entities.iter().any(|e| e.sid == s)
        {
            self.selection = None;
        }
        self.touch();
    }

    fn touch(&mut self) {
        self.revision += 1;
        self.unsynced = true;
    }

    pub fn dirty(&self) -> bool {
        self.scene != self.saved
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.scene.clone();
    }

    pub fn select(&mut self, sid: Option<u64>) {
        let sid = sid.filter(|s| self.scene.entities.iter().any(|e| e.sid == *s));
        if self.selection != sid {
            self.selection = sid;
            self.revision += 1;
        }
    }

    /// Runs an edit on a copy of the scene; on success records the
    /// difference as one transaction.
    fn edit(
        &mut self,
        label: &str,
        key: Option<String>,
        f: impl FnOnce(&mut Scene, &Schemas) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut next = self.scene.clone();
        f(&mut next, &self.schemas)?;
        let diff = SceneDiff::between(&self.scene, &next);
        if diff.is_empty() {
            return Ok(());
        }
        diff.apply(&mut self.scene, true);
        self.history.record(label, diff, key);
        self.touch();
        Ok(())
    }

    fn entity_mut(scene: &mut Scene, sid: u64) -> Result<&mut SceneEntity, String> {
        scene
            .entities
            .iter_mut()
            .find(|e| e.sid == sid)
            .ok_or_else(|| format!("no entity #{sid}"))
    }

    /// Sets one field from text. Edits to the same field merge into one
    /// undo step until [`Document::end_gesture`].
    pub fn set_field(
        &mut self,
        sid: u64,
        component: &str,
        field: &str,
        text: &str,
    ) -> Result<(), String> {
        let key = format!("{sid}/{component}/{field}");
        self.edit(
            &format!("Set {}.{field}", short(component)),
            Some(key),
            |scene, _| {
                let e = Self::entity_mut(scene, sid)?;
                let c = e
                    .components
                    .iter_mut()
                    .find(|c| c.path == component)
                    .ok_or_else(|| format!("#{sid} has no {component}"))?;
                let slot = c
                    .fields
                    .iter_mut()
                    .find(|(n, _)| n == field)
                    .ok_or_else(|| format!("{component} has no field {field}"))?;
                slot.1 = parse_value(slot.1.kind(), text)?;
                Ok(())
            },
        )
    }

    /// Sets several fields of one component in one step (moving an entity
    /// sets x and y together). Merges with the same call until the gesture
    /// ends.
    pub fn set_fields(
        &mut self,
        sid: u64,
        component: &str,
        fields: &[(&str, &str)],
    ) -> Result<(), String> {
        let key = format!("{sid}/{component}/*");
        self.edit(
            &format!("Change {}", short(component)),
            Some(key),
            |scene, _| {
                let e = Self::entity_mut(scene, sid)?;
                let c = e
                    .components
                    .iter_mut()
                    .find(|c| c.path == component)
                    .ok_or_else(|| format!("#{sid} has no {component}"))?;
                for (field, text) in fields {
                    let slot = c
                        .fields
                        .iter_mut()
                        .find(|(n, _)| n == field)
                        .ok_or_else(|| format!("{component} has no field {field}"))?;
                    slot.1 = parse_value(slot.1.kind(), text)?;
                }
                Ok(())
            },
        )
    }

    /// Makes `scene` the document's scene as one undoable step (what running
    /// a stray stack did to the world becomes an edit).
    pub fn replace_scene(&mut self, label: &str, scene: Scene) {
        let diff = SceneDiff::between(&self.scene, &scene);
        if diff.is_empty() {
            return;
        }
        diff.apply(&mut self.scene, true);
        self.history.record(label, diff, None);
        self.touch();
    }

    /// Ends a gesture (a drag, a typing burst): the next edit is a new step.
    pub fn end_gesture(&mut self) {
        self.history.seal();
    }

    pub fn rename(&mut self, sid: u64, name: &str) -> Result<(), String> {
        let name = name.trim().to_string();
        self.edit("Rename", Some(format!("{sid}/name")), |scene, _| {
            Self::entity_mut(scene, sid)?.name = (!name.is_empty()).then_some(name);
            Ok(())
        })
    }

    /// Components (with fields) or tags the entity does not have yet.
    pub fn addable(&self, sid: u64) -> Vec<String> {
        let Some(e) = self.scene.entities.iter().find(|e| e.sid == sid) else {
            return vec![];
        };
        self.schemas
            .keys()
            .filter(|p| !e.components.iter().any(|c| &c.path == *p) && !e.tags.contains(p))
            .cloned()
            .collect()
    }

    pub fn add_component(&mut self, sid: u64, path: &str) -> Result<(), String> {
        self.edit(&format!("Add {}", short(path)), None, |scene, schemas| {
            let schema = schemas
                .get(path)
                .ok_or_else(|| format!("{path} is not a known component"))?
                .clone();
            let e = Self::entity_mut(scene, sid)?;
            if e.components.iter().any(|c| c.path == path) || e.tags.iter().any(|t| t == path) {
                return Err(format!("#{sid} already has {path}"));
            }
            if schema.fields.is_empty() {
                e.tags.push(path.to_string());
                e.tags.sort();
            } else {
                e.components.push(SceneComponent {
                    path: path.to_string(),
                    fields: schema
                        .fields
                        .iter()
                        .map(|(n, k)| (n.clone(), Value::zero(*k)))
                        .collect(),
                });
                e.components.sort_by(|a, b| a.path.cmp(&b.path));
            }
            scene.schemas.entry(path.to_string()).or_insert(schema);
            Ok(())
        })
    }

    pub fn remove_component(&mut self, sid: u64, path: &str) -> Result<(), String> {
        self.edit(&format!("Remove {}", short(path)), None, |scene, _| {
            let e = Self::entity_mut(scene, sid)?;
            let n = e.components.len() + e.tags.len();
            e.components.retain(|c| c.path != path);
            e.tags.retain(|t| t != path);
            if n == e.components.len() + e.tags.len() {
                return Err(format!("#{sid} has no {path}"));
            }
            Ok(())
        })
    }

    /// A new, empty entity (a child of `parent` when given). Returns its
    /// StableId and selects it.
    pub fn spawn(&mut self, name: &str, parent: Option<u64>) -> Result<u64, String> {
        let sid = self.next_sid();
        let name = name.trim().to_string();
        self.edit("Add entity", None, |scene, _| {
            let mut e = SceneEntity {
                sid,
                name: (!name.is_empty()).then_some(name),
                components: vec![],
                tags: vec![],
                pairs: vec![],
            };
            if let Some(p) = parent {
                if !scene.entities.iter().any(|x| x.sid == p) {
                    return Err(format!("no entity #{p}"));
                }
                e.pairs.push(sage_scene::ScenePair {
                    rel: "ChildOf".into(),
                    target_sid: p,
                    target_path: String::new(),
                });
            }
            scene.entities.push(e);
            scene.entities.sort_by_key(|e| e.sid);
            Ok(())
        })?;
        self.select(Some(sid));
        Ok(sid)
    }

    fn next_sid(&self) -> u64 {
        self.scene.entities.iter().map(|e| e.sid).max().unwrap_or(0) + 1
    }

    /// Deletes an entity and its descendants.
    pub fn despawn(&mut self, sid: u64) -> Result<(), String> {
        let mut doomed = vec![sid];
        let mut i = 0;
        while i < doomed.len() {
            let at = doomed[i];
            for e in &self.scene.entities {
                if e.pairs
                    .iter()
                    .any(|p| is_child_of(&p.rel) && p.target_sid == at)
                    && !doomed.contains(&e.sid)
                {
                    doomed.push(e.sid);
                }
            }
            i += 1;
        }
        self.edit("Delete entity", None, |scene, _| {
            if !scene.entities.iter().any(|e| e.sid == sid) {
                return Err(format!("no entity #{sid}"));
            }
            scene.entities.retain(|e| !doomed.contains(&e.sid));
            Ok(())
        })?;
        if self.selection.is_some_and(|s| doomed.contains(&s)) {
            self.selection = None;
        }
        Ok(())
    }

    /// Copies an entity (components, tags and pairs) and selects the copy.
    pub fn duplicate(&mut self, sid: u64) -> Result<u64, String> {
        let new = self.next_sid();
        self.edit("Duplicate entity", None, |scene, _| {
            let mut c = scene
                .entities
                .iter()
                .find(|e| e.sid == sid)
                .ok_or_else(|| format!("no entity #{sid}"))?
                .clone();
            c.sid = new;
            c.name = c.name.map(|n| format!("{n} copy"));
            scene.entities.push(c);
            scene.entities.sort_by_key(|e| e.sid);
            Ok(())
        })?;
        self.select(Some(new));
        Ok(new)
    }

    pub fn undo(&mut self) -> Option<String> {
        let l = self.history.undo(&mut self.scene)?;
        self.fix_selection();
        self.touch();
        Some(l)
    }

    pub fn redo(&mut self) -> Option<String> {
        let l = self.history.redo(&mut self.scene)?;
        self.fix_selection();
        self.touch();
        Some(l)
    }

    fn fix_selection(&mut self) {
        if let Some(s) = self.selection
            && !self.scene.entities.iter().any(|e| e.sid == s)
        {
            self.selection = None;
        }
    }

    pub fn inspect(&self, sid: u64) -> Option<EntityView> {
        inspect(&self.scene, sid)
    }

    pub fn outliner(&self) -> Vec<OutlinerRow> {
        outliner(&self.scene)
    }
}
