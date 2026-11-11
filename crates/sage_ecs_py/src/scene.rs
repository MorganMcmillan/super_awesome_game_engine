//! Scenes and save games of a live world (`sage_scene` formats, design doc
//! 10.4). A scene is the snapshot's entities and singletons written field
//! by field, so files stay readable and can be migrated; a save game adds
//! what a scene does not carry (the RNG, the StableId counter, flecs module
//! switches, and host extras such as a contact table).

use std::collections::{BTreeMap, BTreeSet};

use sage_core::Rng;
use sage_scene::{
    Mode, Policy, Save, SaveMeta, Scene, SceneComponent, SceneEntity, ScenePair, Schemas,
    StoredSchema, Value,
};

use crate::registry::{FieldInfo, Kind, Layout, Registry};
use crate::state::{self, EntitySnapshot, Snapshot, StoredValue};

fn kind_of(k: Kind) -> sage_scene::Kind {
    match k {
        Kind::Bool => sage_scene::Kind::Bool,
        Kind::U8 => sage_scene::Kind::U8,
        Kind::I32 => sage_scene::Kind::I32,
        Kind::U32 => sage_scene::Kind::U32,
        Kind::I64 => sage_scene::Kind::I64,
        Kind::U64 => sage_scene::Kind::U64,
        Kind::F32 => sage_scene::Kind::F32,
        Kind::F64 => sage_scene::Kind::F64,
        Kind::Entity => sage_scene::Kind::Entity,
    }
}

/// Built-in Flecs ids are written as `flecs:Name` and have no schema.
fn is_builtin(path: &str) -> bool {
    path.starts_with("flecs:")
}

/// The schema of every reflected component in this world, at the version
/// its migrations lead to.
pub fn schemas(reg: &Registry) -> Schemas {
    let migrations = reg.migrations.borrow();
    reg.types
        .borrow()
        .values()
        .filter_map(|t| {
            let fields = match &t.layout {
                Layout::Struct(f) => f
                    .iter()
                    .map(|f| (f.name.clone(), kind_of(f.kind)))
                    .collect(),
                Layout::Tag => vec![],
                Layout::Opaque => return None,
            };
            Some((
                t.path.clone(),
                StoredSchema {
                    version: migrations.latest_version(&t.path),
                    fields,
                },
            ))
        })
        .collect()
}

fn read(kind: Kind, b: &[u8]) -> Value {
    match kind {
        Kind::Bool => Value::Bool(b[0] != 0),
        Kind::U8 => Value::U8(b[0]),
        Kind::I32 => Value::I32(i32::from_ne_bytes(b[..4].try_into().unwrap())),
        Kind::U32 => Value::U32(u32::from_ne_bytes(b[..4].try_into().unwrap())),
        Kind::I64 => Value::I64(i64::from_ne_bytes(b[..8].try_into().unwrap())),
        Kind::U64 => Value::U64(u64::from_ne_bytes(b[..8].try_into().unwrap())),
        Kind::F32 => Value::F32(f32::from_ne_bytes(b[..4].try_into().unwrap())),
        Kind::F64 => Value::F64(f64::from_ne_bytes(b[..8].try_into().unwrap())),
        // A snapshot holds the target's StableId, not a Flecs id.
        Kind::Entity => Value::Entity(u64::from_ne_bytes(b[..8].try_into().unwrap())),
    }
}

fn write(v: &Value, out: &mut [u8]) {
    match *v {
        Value::Bool(x) => out[0] = x as u8,
        Value::U8(x) => out[0] = x,
        Value::I32(x) => out[..4].copy_from_slice(&x.to_ne_bytes()),
        Value::U32(x) => out[..4].copy_from_slice(&x.to_ne_bytes()),
        Value::I64(x) => out[..8].copy_from_slice(&x.to_ne_bytes()),
        Value::U64(x) | Value::Entity(x) => out[..8].copy_from_slice(&x.to_ne_bytes()),
        Value::F32(x) => out[..4].copy_from_slice(&x.to_ne_bytes()),
        Value::F64(x) => out[..8].copy_from_slice(&x.to_ne_bytes()),
    }
}

/// Layouts by component path: (size, fields).
fn layouts(reg: &Registry) -> BTreeMap<String, (usize, Vec<FieldInfo>)> {
    reg.types
        .borrow()
        .values()
        .filter_map(|t| match &t.layout {
            Layout::Struct(f) => Some((t.path.clone(), (t.size, f.clone()))),
            _ => None,
        })
        .collect()
}

fn decode(layouts: &BTreeMap<String, (usize, Vec<FieldInfo>)>, v: &StoredValue) -> SceneComponent {
    let (_, fields) = &layouts[&v.path];
    SceneComponent {
        path: v.path.clone(),
        fields: fields
            .iter()
            .map(|f| (f.name.clone(), read(f.kind, &v.bytes[f.offset..])))
            .collect(),
    }
}

fn encode(
    layouts: &BTreeMap<String, (usize, Vec<FieldInfo>)>,
    c: &SceneComponent,
) -> Result<StoredValue, String> {
    let (size, fields) = layouts
        .get(&c.path)
        .ok_or_else(|| format!("{} is not a data component in this world", c.path))?;
    let mut bytes = vec![0u8; *size];
    for f in fields {
        let (_, v) = c
            .fields
            .iter()
            .find(|(n, _)| *n == f.name)
            .ok_or_else(|| format!("{}: missing field {}", c.path, f.name))?;
        if v.kind() != kind_of(f.kind) {
            return Err(format!(
                "{}.{}: expected {}, found {}",
                c.path,
                f.name,
                kind_of(f.kind).name(),
                v.kind().name()
            ));
        }
        write(v, &mut bytes[f.offset..]);
    }
    Ok(StoredValue {
        path: c.path.clone(),
        bytes,
    })
}

/// The snapshot as a scene.
pub fn to_scene(reg: &Registry, snap: &Snapshot) -> Scene {
    let layouts = layouts(reg);
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut scene = Scene::new(Mode::TwoD);
    for es in &snap.entities {
        let components: Vec<_> = es.values.iter().map(|v| decode(&layouts, v)).collect();
        used.extend(components.iter().map(|c| c.path.clone()));
        used.extend(es.tags.iter().cloned());
        used.extend(es.pairs.iter().map(|p| p.0.clone()));
        scene.entities.push(SceneEntity {
            sid: es.sid,
            name: es.name.clone(),
            components,
            tags: es.tags.clone(),
            pairs: es
                .pairs
                .iter()
                .map(|(rel, sid, path)| ScenePair {
                    rel: rel.clone(),
                    target_sid: *sid,
                    target_path: path.clone(),
                })
                .collect(),
        });
    }
    for v in &snap.singletons {
        used.insert(v.path.clone());
        scene.singletons.push(decode(&layouts, v));
    }
    let all = schemas(reg);
    scene.schemas = used
        .into_iter()
        .filter(|p| !is_builtin(p))
        .filter_map(|p| all.get(&p).map(|s| (p, s.clone())))
        .collect();
    scene.strings = snap.strings.clone();
    scene
}

/// A snapshot holding the scene's entities and singletons, with everything
/// else (flecs module switches, RNG, counter) taken from the world as it is.
fn from_scene(reg: &Registry, scene: &Scene) -> Result<Snapshot, String> {
    let layouts = layouts(reg);
    let current = state::snapshot(reg);
    let mut entities = vec![];
    for e in &scene.entities {
        entities.push(EntitySnapshot {
            sid: e.sid,
            name: e.name.clone(),
            values: e
                .components
                .iter()
                .map(|c| encode(&layouts, c))
                .collect::<Result<_, _>>()?,
            tags: e.tags.clone(),
            pairs: e
                .pairs
                .iter()
                .map(|p| (p.rel.clone(), p.target_sid, p.target_path.clone()))
                .collect(),
        });
    }
    let singletons = scene
        .singletons
        .iter()
        .map(|c| encode(&layouts, c))
        .collect::<Result<_, _>>()?;
    let highest = scene.entities.iter().map(|e| e.sid).max().unwrap_or(0);
    Ok(Snapshot {
        entities,
        singletons,
        modules: current.modules,
        next_sid: current.next_sid.max(highest + 1),
        rng: current.rng,
        strings: scene.strings.clone(),
    })
}

fn bring_up_to_date(
    reg: &Registry,
    scene: &mut Scene,
    policy: Policy,
) -> Result<Vec<String>, String> {
    let mut report = scene
        .migrate(&reg.migrations.borrow())
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|m| format!("migrated {m}"))
        .collect::<Vec<_>>();
    report.extend(scene.conform(&schemas(reg), policy)?);
    Ok(report)
}

pub fn save_scene(reg: &Registry) -> String {
    to_scene(reg, &state::snapshot(reg)).write()
}

/// Replaces the world's entities and singletons with the scene's, after
/// migrating it. Strings the world already knows are kept. Returns what was
/// migrated and any lenient-policy warnings.
pub fn load_scene(reg: &Registry, text: &str, policy: Policy) -> Result<Vec<String>, String> {
    let mut scene = Scene::parse(text).map_err(|e| e.to_string())?;
    let report = bring_up_to_date(reg, &mut scene, policy)?;
    let mut snap = from_scene(reg, &scene)?;
    let mut merged = reg.strings.borrow().clone();
    for (_, s) in scene.strings.iter() {
        merged.intern(s);
    }
    snap.strings = merged;
    state::restore(reg, &snap)?;
    Ok(report)
}

pub fn save_game(reg: &Registry, extra: BTreeMap<String, String>) -> String {
    let snap = state::snapshot(reg);
    Save {
        scene: to_scene(reg, &snap),
        meta: SaveMeta {
            next_sid: snap.next_sid,
            rng: snap.rng.state(),
            modules: snap.modules.clone(),
            extra,
        },
    }
    .write()
}

/// What a loaded save game hands back to the host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadedGame {
    /// Migrations applied and warnings.
    pub report: Vec<String>,
    /// The host extras written by `save_game`.
    pub extra: BTreeMap<String, String>,
}

pub fn load_game(reg: &Registry, text: &str, policy: Policy) -> Result<LoadedGame, String> {
    let save = Save::parse(text).map_err(|e| e.to_string())?;
    let mut scene = save.scene;
    let report = bring_up_to_date(reg, &mut scene, policy)?;
    let mut snap = from_scene(reg, &scene)?;
    // The saved counter is exact; it only has to stay above every StableId.
    let highest = scene.entities.iter().map(|e| e.sid).max().unwrap_or(0);
    snap.next_sid = save.meta.next_sid.max(highest + 1);
    snap.rng = Rng::from_state(save.meta.rng).ok_or("the save game's RNG state is all zero")?;
    snap.modules = save.meta.modules;
    state::restore(reg, &snap)?;
    Ok(LoadedGame {
        report,
        extra: save.meta.extra,
    })
}
