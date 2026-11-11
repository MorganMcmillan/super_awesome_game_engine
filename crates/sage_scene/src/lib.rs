//! Scenes, save games and schema migrations (design doc 10.4).
//!
//! This crate is pure data and has no dependencies. It defines:
//! - [`Scene`]: entities, their components field by field, singletons and a
//!   string table, with the schema (version, field names and kinds) of every
//!   component the file mentions;
//! - [`Save`]: a scene plus the world state a scene does not carry (the RNG,
//!   the StableId counter, flecs module switches and host extras);
//! - the deterministic text formats `sage-scene v1` and `sage-save v1`
//!   (module `text`);
//! - [`Migrations`] (module `migrate`): ordered, per-component steps that bring
//!   an old file up to the schema the world reflects now.
//!
//! The binding (`sage_ecs_py`) converts between these and a live world.
//! Entities are identified by StableId, never by Flecs ids.

mod migrate;
mod text;

use std::collections::BTreeMap;
use std::fmt;

pub use migrate::{MigrateError, Migrations, Op, Step};
pub use text::{SceneError, SchemaSpec, hash_str, schemas_from};

/// The primitive field types of a reflected component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bool,
    U8,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    /// An entity reference, stored as the target's StableId (0 = none).
    Entity,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Bool => "bool",
            Kind::U8 => "u8",
            Kind::I32 => "i32",
            Kind::U32 => "u32",
            Kind::I64 => "i64",
            Kind::U64 => "u64",
            Kind::F32 => "f32",
            Kind::F64 => "f64",
            Kind::Entity => "entity",
        }
    }

    pub fn from_name(s: &str) -> Option<Kind> {
        Some(match s {
            "bool" => Kind::Bool,
            "u8" => Kind::U8,
            "i32" => Kind::I32,
            "u32" => Kind::U32,
            "i64" => Kind::I64,
            "u64" => Kind::U64,
            "f32" => Kind::F32,
            "f64" => Kind::F64,
            "entity" => Kind::Entity,
            _ => return None,
        })
    }
}

/// One field's value.
#[derive(Clone, Copy, Debug)]
pub enum Value {
    Bool(bool),
    U8(u8),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    Entity(u64),
}

impl PartialEq for Value {
    /// Floats compare by bit pattern, so a scene equals itself after a text
    /// round trip even when it holds a NaN.
    fn eq(&self, other: &Value) -> bool {
        use Value::*;
        match (*self, *other) {
            (Bool(a), Bool(b)) => a == b,
            (U8(a), U8(b)) => a == b,
            (I32(a), I32(b)) => a == b,
            (U32(a), U32(b)) => a == b,
            (I64(a), I64(b)) => a == b,
            (U64(a), U64(b)) => a == b,
            (F32(a), F32(b)) => a.to_bits() == b.to_bits(),
            (F64(a), F64(b)) => a.to_bits() == b.to_bits(),
            (Entity(a), Entity(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    pub fn kind(&self) -> Kind {
        match self {
            Value::Bool(_) => Kind::Bool,
            Value::U8(_) => Kind::U8,
            Value::I32(_) => Kind::I32,
            Value::U32(_) => Kind::U32,
            Value::I64(_) => Kind::I64,
            Value::U64(_) => Kind::U64,
            Value::F32(_) => Kind::F32,
            Value::F64(_) => Kind::F64,
            Value::Entity(_) => Kind::Entity,
        }
    }

    pub fn zero(kind: Kind) -> Value {
        match kind {
            Kind::Bool => Value::Bool(false),
            Kind::U8 => Value::U8(0),
            Kind::I32 => Value::I32(0),
            Kind::U32 => Value::U32(0),
            Kind::I64 => Value::I64(0),
            Kind::U64 => Value::U64(0),
            Kind::F32 => Value::F32(0.0),
            Kind::F64 => Value::F64(0.0),
            Kind::Entity => Value::Entity(0),
        }
    }

    /// Converts to another kind. Integers must fit; floats convert to
    /// integers only when finite, whole and in range; bool is "non-zero".
    pub fn convert(&self, to: Kind) -> Result<Value, String> {
        if self.kind() == to {
            return Ok(*self);
        }
        enum Num {
            Int(i128),
            Float(f64),
        }
        let n = match *self {
            Value::Bool(b) => Num::Int(b as i128),
            Value::U8(v) => Num::Int(v as i128),
            Value::I32(v) => Num::Int(v as i128),
            Value::U32(v) => Num::Int(v as i128),
            Value::I64(v) => Num::Int(v as i128),
            Value::U64(v) | Value::Entity(v) => Num::Int(v as i128),
            Value::F32(v) => Num::Float(v as f64),
            Value::F64(v) => Num::Float(v),
        };
        let fail = || format!("{self} does not convert to {}", to.name());
        let int = |lo: i128, hi: i128| -> Result<i128, String> {
            let v = match n {
                Num::Int(v) => v,
                Num::Float(f) => {
                    if !f.is_finite() || f.fract() != 0.0 {
                        return Err(fail());
                    }
                    f as i128
                }
            };
            if v < lo || v > hi {
                return Err(fail());
            }
            Ok(v)
        };
        Ok(match to {
            Kind::Bool => Value::Bool(match n {
                Num::Int(v) => v != 0,
                Num::Float(f) => f != 0.0,
            }),
            Kind::U8 => Value::U8(int(0, u8::MAX as i128)? as u8),
            Kind::I32 => Value::I32(int(i32::MIN as i128, i32::MAX as i128)? as i32),
            Kind::U32 => Value::U32(int(0, u32::MAX as i128)? as u32),
            Kind::I64 => Value::I64(int(i64::MIN as i128, i64::MAX as i128)? as i64),
            Kind::U64 => Value::U64(int(0, u64::MAX as i128)? as u64),
            Kind::Entity => Value::Entity(int(0, u64::MAX as i128)? as u64),
            Kind::F32 => Value::F32(match n {
                Num::Int(v) => v as f32,
                Num::Float(f) => f as f32,
            }),
            Kind::F64 => Value::F64(match n {
                Num::Int(v) => v as f64,
                Num::Float(f) => f,
            }),
        })
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&text::format_value(self))
    }
}

/// A component's schema: version, and fields in declaration order. A tag has
/// no fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredSchema {
    pub version: u32,
    pub fields: Vec<(String, Kind)>,
}

/// Schemas by component path.
pub type Schemas = BTreeMap<String, StoredSchema>;

/// How strictly a scene must match the schema the world reflects now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Any difference after migration is an error. The default.
    Strict,
    /// Fields the world lacks are dropped, fields the scene lacks are zero,
    /// and each is reported as a warning. Unknown components are still errors.
    Lenient,
}

/// Whether a scene is two- or three-dimensional (design doc 10.1: the scene
/// carries the mode, not each entity).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    TwoD,
    ThreeD,
}

/// A component value on an entity or a singleton: fields in schema order.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneComponent {
    pub path: String,
    pub fields: Vec<(String, Value)>,
}

/// A relationship pair on an entity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenePair {
    pub rel: String,
    /// The target's StableId, or 0 when the target has none.
    pub target_sid: u64,
    /// The target's path when it has no StableId (a prefab-less built-in).
    pub target_path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneEntity {
    pub sid: u64,
    pub name: Option<String>,
    /// Sorted by path.
    pub components: Vec<SceneComponent>,
    /// Tag paths, sorted.
    pub tags: Vec<String>,
    /// Sorted by (rel, target_sid, target_path).
    pub pairs: Vec<ScenePair>,
}

/// Interned strings by content hash, so text lives in scenes and save games
/// and components store only the 64-bit id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StringTable {
    map: BTreeMap<u64, String>,
}

impl StringTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// The id of `s`: its content hash, with the empty string as 0.
    /// Panics on a 64-bit hash collision between different strings.
    pub fn intern(&mut self, s: &str) -> u64 {
        let id = hash_str(s);
        if id == 0 {
            return 0;
        }
        match self.map.get(&id) {
            Some(old) => assert_eq!(old, s, "string hash collision: {old:?} and {s:?}"),
            None => {
                self.map.insert(id, s.to_string());
            }
        }
        id
    }

    pub fn get(&self, id: u64) -> Option<&str> {
        if id == 0 {
            return Some("");
        }
        self.map.get(&id).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// In id order.
    pub fn iter(&self) -> impl Iterator<Item = (u64, &str)> {
        self.map.iter().map(|(k, v)| (*k, v.as_str()))
    }

    pub fn hash_into(&self, h: &mut dyn FnMut(&[u8])) {
        for (id, s) in self.iter() {
            h(&id.to_le_bytes());
            h(&(s.len() as u64).to_le_bytes());
            h(s.as_bytes());
        }
    }
}

/// A set of entities saved as a unit (design doc 14).
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub mode: Mode,
    /// The schema every component in the file was written with.
    pub schemas: Schemas,
    /// By StableId.
    pub entities: Vec<SceneEntity>,
    /// Sorted by path.
    pub singletons: Vec<SceneComponent>,
    pub strings: StringTable,
}

impl Scene {
    pub fn new(mode: Mode) -> Scene {
        Scene {
            mode,
            schemas: Schemas::new(),
            entities: vec![],
            singletons: vec![],
            strings: StringTable::new(),
        }
    }

    /// The scene in the deterministic `sage-scene v1` text form.
    pub fn write(&self) -> String {
        text::write_scene(self, None)
    }

    pub fn parse(src: &str) -> Result<Scene, SceneError> {
        text::parse(src).and_then(|d| match d {
            text::Document::Scene(s) => Ok(s),
            text::Document::Save(_) => Err(SceneError::new(1, "this is a save game, not a scene")),
        })
    }

    /// Brings every component up to its latest version (see [`Migrations`]).
    pub fn migrate(&mut self, migrations: &Migrations) -> Result<Vec<String>, MigrateError> {
        migrate::apply(self, migrations)
    }

    /// Checks the scene against the schema the world reflects now and
    /// rewrites it to match. Call after [`Scene::migrate`]. Returns warnings
    /// (always empty under [`Policy::Strict`], which fails instead).
    pub fn conform(&mut self, live: &Schemas, policy: Policy) -> Result<Vec<String>, String> {
        migrate::conform(self, live, policy)
    }
}

/// State a scene does not carry but a save game needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveMeta {
    pub next_sid: u64,
    pub rng: [u64; 4],
    /// Flecs module paths and whether each is enabled, sorted by path.
    pub modules: Vec<(String, bool)>,
    /// Host state the binding does not know (a contact table, say), by key.
    pub extra: BTreeMap<String, String>,
}

/// A scene plus world state: `sage-save v1`.
#[derive(Clone, Debug, PartialEq)]
pub struct Save {
    pub scene: Scene,
    pub meta: SaveMeta,
}

impl Save {
    pub fn write(&self) -> String {
        text::write_scene(&self.scene, Some(&self.meta))
    }

    pub fn parse(src: &str) -> Result<Save, SceneError> {
        text::parse(src).and_then(|d| match d {
            text::Document::Save(s) => Ok(s),
            text::Document::Scene(_) => Err(SceneError::new(1, "this is a scene, not a save game")),
        })
    }
}

#[cfg(test)]
mod tests;
