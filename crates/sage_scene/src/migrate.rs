//! Schema versions and migrations (design doc 10.4).
//!
//! Every component in a file is stored with the version it was written at,
//! together with its field names and kinds. A [`Migration`] step takes one
//! component from version `n` to `n + 1`; steps ship with the component's
//! flecs module or kit, as data, in the `sage-migrations v1` text form:
//!
//! ```text
//! sage-migrations v1
//! migrate pong.Player 1          the step from version 1 to 2
//!   rename-field score points
//!   add-field lives u32 3
//!   drop-field legacy
//!   convert-field speed f64
//!   rename-component pong.Contestant
//! ```
//!
//! A step is keyed by the component's path *as it was at that version*, so
//! a rename ends one chain and the new name carries on from the next
//! version. The latest version of a path is one more than the highest step
//! that ends at it. The engine cannot tell a rename from a drop plus an add
//! (the editor asks), so renames are always written down.

use std::collections::BTreeMap;
use std::fmt;

use crate::text::{format_value, parse_value};
use crate::{Kind, Policy, Scene, SceneComponent, ScenePair, Schemas, StoredSchema, Value};

#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    RenameField {
        from: String,
        to: String,
    },
    /// The new field goes last.
    AddField {
        name: String,
        default: Value,
    },
    DropField {
        name: String,
    },
    ConvertField {
        name: String,
        to: Kind,
    },
    /// The component's path changes; its version carries on.
    RenameComponent {
        to: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// The component's path at version `from`.
    pub path: String,
    pub from: u32,
    pub ops: Vec<Op>,
}

impl Step {
    fn end_path(&self) -> &str {
        self.ops
            .iter()
            .rev()
            .find_map(|o| match o {
                Op::RenameComponent { to } => Some(to.as_str()),
                _ => None,
            })
            .unwrap_or(&self.path)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MigrateError {
    /// A migrations file did not parse.
    Parse { line: usize, message: String },
    /// A step is defined twice for the same path and version.
    Duplicate { path: String, from: u32 },
    /// A step does not apply to the component it names.
    Step {
        path: String,
        from: u32,
        message: String,
    },
    /// A value does not survive a conversion.
    Value {
        path: String,
        entity: Option<u64>,
        message: String,
    },
    /// Two components migrate to the same path with different schemas.
    Collision { path: String },
}

impl fmt::Display for MigrateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrateError::Parse { line, message } => write!(f, "migrations line {line}: {message}"),
            MigrateError::Duplicate { path, from } => {
                write!(f, "two migrations for {path} from version {from}")
            }
            MigrateError::Step {
                path,
                from,
                message,
            } => write!(f, "migration of {path} from version {from}: {message}"),
            MigrateError::Value {
                path,
                entity: Some(e),
                message,
            } => write!(f, "entity {e}, {path}: {message}"),
            MigrateError::Value {
                path,
                entity: None,
                message,
            } => write!(f, "singleton {path}: {message}"),
            MigrateError::Collision { path } => write!(
                f,
                "migrating to {path} collides with a component of that name that has a different schema"
            ),
        }
    }
}

impl std::error::Error for MigrateError {}

/// All migration steps known to a world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Migrations {
    steps: BTreeMap<(String, u32), Step>,
}

impl Migrations {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn add(&mut self, step: Step) -> Result<(), MigrateError> {
        let key = (step.path.clone(), step.from);
        if self.steps.contains_key(&key) {
            return Err(MigrateError::Duplicate {
                path: step.path,
                from: step.from,
            });
        }
        self.steps.insert(key, step);
        Ok(())
    }

    pub fn steps(&self) -> impl Iterator<Item = &Step> {
        self.steps.values()
    }

    /// The version a component at `path` is written at now.
    pub fn latest_version(&self, path: &str) -> u32 {
        self.steps
            .values()
            .filter(|s| s.end_path() == path)
            .map(|s| s.from + 1)
            .max()
            .unwrap_or(1)
    }

    /// Reads the `sage-migrations v1` form and adds its steps.
    pub fn add_text(&mut self, src: &str) -> Result<(), MigrateError> {
        let err = |line: usize, message: String| MigrateError::Parse { line, message };
        let mut lines = src
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l))
            .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
        match lines.next() {
            Some((_, l)) if l.trim() == "sage-migrations v1" => {}
            Some((n, l)) => {
                return Err(err(
                    n,
                    format!("expected `sage-migrations v1`, found {l:?}"),
                ));
            }
            None => return Err(err(1, "empty file".into())),
        }
        let mut current: Option<Step> = None;
        for (n, raw) in lines {
            let w: Vec<&str> = raw.split_whitespace().collect();
            if !(raw.starts_with(' ') || raw.starts_with('\t')) {
                if let Some(s) = current.take() {
                    self.add(s)?;
                }
                if w.len() != 3 || w[0] != "migrate" {
                    return Err(err(n, "expected `migrate <path> <from version>`".into()));
                }
                let from = w[2]
                    .parse()
                    .map_err(|_| err(n, format!("bad version {:?}", w[2])))?;
                current = Some(Step {
                    path: w[1].to_string(),
                    from,
                    ops: vec![],
                });
                continue;
            }
            let step = current
                .as_mut()
                .ok_or_else(|| err(n, "operation before any `migrate` line".into()))?;
            let kind = |s: &str| {
                Kind::from_name(s).ok_or_else(|| err(n, format!("unknown field kind {s:?}")))
            };
            let op = match (w[0], w.len()) {
                ("rename-field", 3) => Op::RenameField {
                    from: w[1].into(),
                    to: w[2].into(),
                },
                ("add-field", 4) => {
                    let k = kind(w[2])?;
                    Op::AddField {
                        name: w[1].into(),
                        default: parse_value(k, w[3]).map_err(|m| err(n, m))?,
                    }
                }
                ("drop-field", 2) => Op::DropField { name: w[1].into() },
                ("convert-field", 3) => Op::ConvertField {
                    name: w[1].into(),
                    to: kind(w[2])?,
                },
                ("rename-component", 2) => Op::RenameComponent { to: w[1].into() },
                _ => {
                    return Err(err(
                        n,
                        format!("unknown or malformed operation {:?}", raw.trim()),
                    ));
                }
            };
            step.ops.push(op);
        }
        if let Some(s) = current.take() {
            self.add(s)?;
        }
        Ok(())
    }

    pub fn parse(src: &str) -> Result<Migrations, MigrateError> {
        let mut m = Migrations::new();
        m.add_text(src)?;
        Ok(m)
    }

    /// The `sage-migrations v1` text, steps sorted by path and version.
    pub fn write(&self) -> String {
        let mut out = String::from("sage-migrations v1\n");
        for s in self.steps.values() {
            out.push_str(&format!("migrate {} {}\n", s.path, s.from));
            for op in &s.ops {
                match op {
                    Op::RenameField { from, to } => {
                        out.push_str(&format!("  rename-field {from} {to}\n"))
                    }
                    Op::AddField { name, default } => out.push_str(&format!(
                        "  add-field {name} {} {}\n",
                        default.kind().name(),
                        format_value(default)
                    )),
                    Op::DropField { name } => out.push_str(&format!("  drop-field {name}\n")),
                    Op::ConvertField { name, to } => {
                        out.push_str(&format!("  convert-field {name} {}\n", to.name()))
                    }
                    Op::RenameComponent { to } => {
                        out.push_str(&format!("  rename-component {to}\n"))
                    }
                }
            }
        }
        out
    }
}

type Fields = Vec<(String, Value)>;

fn apply_ops(ops: &[Op], fields: &mut Fields, path: &mut String) -> Result<(), String> {
    for op in ops {
        match op {
            Op::RenameField { from, to } => {
                if fields.iter().any(|(n, _)| n == to) {
                    return Err(format!("rename-field: {to} already exists"));
                }
                let f = fields
                    .iter_mut()
                    .find(|(n, _)| n == from)
                    .ok_or_else(|| format!("rename-field: no field {from}"))?;
                f.0 = to.clone();
            }
            Op::AddField { name, default } => {
                if fields.iter().any(|(n, _)| n == name) {
                    return Err(format!("add-field: {name} already exists"));
                }
                fields.push((name.clone(), *default));
            }
            Op::DropField { name } => {
                let i = fields
                    .iter()
                    .position(|(n, _)| n == name)
                    .ok_or_else(|| format!("drop-field: no field {name}"))?;
                fields.remove(i);
            }
            Op::ConvertField { name, to } => {
                let f = fields
                    .iter_mut()
                    .find(|(n, _)| n == name)
                    .ok_or_else(|| format!("convert-field: no field {name}"))?;
                f.1 = f.1.convert(*to)?;
            }
            Op::RenameComponent { to } => *path = to.clone(),
        }
    }
    Ok(())
}

/// What the chain of steps from `(path, version)` does.
struct Chain {
    steps: Vec<Step>,
    path: String,
    version: u32,
}

fn chain_for(m: &Migrations, path: &str, mut version: u32) -> Chain {
    let mut steps = vec![];
    let mut p = path.to_string();
    while let Some(s) = m.steps.get(&(p.clone(), version)) {
        p = s.end_path().to_string();
        version += 1;
        steps.push(s.clone());
        // A cycle of renames would never end.
        if steps.len() > 10_000 {
            break;
        }
    }
    Chain {
        steps,
        path: p,
        version,
    }
}

fn run_chain(chain: &Chain, path: &str, fields: &mut Fields) -> Result<String, MigrateError> {
    let mut p = path.to_string();
    for s in &chain.steps {
        apply_ops(&s.ops, fields, &mut p).map_err(|message| MigrateError::Step {
            path: s.path.clone(),
            from: s.from,
            message,
        })?;
    }
    Ok(p)
}

pub fn apply(scene: &mut Scene, m: &Migrations) -> Result<Vec<String>, MigrateError> {
    let mut report = vec![];
    // Per old path: its chain and the schema it ends with.
    let mut plans: BTreeMap<String, (Chain, StoredSchema)> = BTreeMap::new();
    let mut new_schemas = Schemas::new();
    for (path, schema) in &scene.schemas {
        let chain = chain_for(m, path, schema.version);
        let mut probe: Fields = schema
            .fields
            .iter()
            .map(|(n, k)| (n.clone(), Value::zero(*k)))
            .collect();
        let new_path = run_chain(&chain, path, &mut probe)?;
        let new = StoredSchema {
            version: chain.version,
            fields: probe.iter().map(|(n, v)| (n.clone(), v.kind())).collect(),
        };
        if !chain.steps.is_empty() {
            report.push(format!(
                "{path} v{} -> {new_path} v{}",
                schema.version, chain.version
            ));
        }
        if new_schemas.get(&new_path).is_some_and(|old| *old != new) {
            return Err(MigrateError::Collision { path: new_path });
        }
        new_schemas.insert(new_path, new.clone());
        plans.insert(path.clone(), (chain, new));
    }
    if report.is_empty() {
        return Ok(report);
    }
    let rename = |path: &str| -> String {
        plans
            .get(path)
            .map(|(c, _)| c.path.clone())
            .unwrap_or_else(|| path.to_string())
    };
    let migrate_component =
        |c: &mut SceneComponent, entity: Option<u64>| -> Result<(), MigrateError> {
            if let Some((chain, _)) = plans.get(&c.path) {
                let new_path = run_chain(chain, &c.path, &mut c.fields).map_err(|e| match e {
                    MigrateError::Step { message, .. } => MigrateError::Value {
                        path: c.path.clone(),
                        entity,
                        message,
                    },
                    other => other,
                })?;
                c.path = new_path;
            }
            Ok(())
        };
    for e in &mut scene.entities {
        for c in &mut e.components {
            migrate_component(c, Some(e.sid))?;
        }
        e.components.sort_by(|a, b| a.path.cmp(&b.path));
        for t in &mut e.tags {
            *t = rename(t);
        }
        e.tags.sort();
        for p in &mut e.pairs {
            p.rel = rename(&p.rel);
        }
        e.pairs.sort_by(|a: &ScenePair, b| {
            (&a.rel, a.target_sid, &a.target_path).cmp(&(&b.rel, b.target_sid, &b.target_path))
        });
    }
    for c in &mut scene.singletons {
        migrate_component(c, None)?;
    }
    scene.singletons.sort_by(|a, b| a.path.cmp(&b.path));
    scene.schemas = new_schemas;
    Ok(report)
}

pub fn conform(scene: &mut Scene, live: &Schemas, policy: Policy) -> Result<Vec<String>, String> {
    let mut warnings = vec![];
    // path -> fields the world no longer has
    let mut fixes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, s) in &scene.schemas {
        let l = live.get(path).ok_or_else(|| {
            format!(
                "the scene has component {path}, which the world does not define \
                 (was it renamed? add a `rename-component` migration)"
            )
        })?;
        if l.version != s.version {
            return Err(format!(
                "{path} is version {} in the scene but version {} in the world, \
                 and no migration leads from one to the other",
                s.version, l.version
            ));
        }
        let mut drop = vec![];
        let mut problems = vec![];
        // A kind change cannot be repaired by dropping or zeroing a field.
        let mut hard = false;
        for (name, kind) in &s.fields {
            match l.fields.iter().find(|(n, _)| n == name) {
                None => {
                    problems.push(format!("the world has no field {name}"));
                    drop.push(name.clone());
                }
                Some((_, k)) if k != kind => {
                    hard = true;
                    problems.push(format!(
                        "field {name} is {} in the scene but {} in the world",
                        kind.name(),
                        k.name()
                    ));
                }
                Some(_) => {}
            }
        }
        for (name, _) in &l.fields {
            if !s.fields.iter().any(|(n, _)| n == name) {
                problems.push(format!("the scene has no field {name}"));
            }
        }
        if problems.is_empty() {
            continue;
        }
        if policy == Policy::Strict || hard {
            return Err(format!(
                "{path} (version {}): {}. Add a migration step, or load with the lenient policy.",
                s.version,
                problems.join("; ")
            ));
        }
        for p in &problems {
            warnings.push(format!("{path}: {p}"));
        }
        fixes.insert(path.clone(), drop);
    }
    let fix = |c: &mut SceneComponent| {
        let live_fields = &live[&c.path].fields;
        if let Some(drop) = fixes.get(&c.path) {
            c.fields.retain(|(n, _)| !drop.contains(n));
        }
        let mut ordered: Fields = vec![];
        for (name, kind) in live_fields {
            let v = c
                .fields
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| *v)
                .unwrap_or(Value::zero(*kind));
            ordered.push((name.clone(), v));
        }
        c.fields = ordered;
    };
    for e in &mut scene.entities {
        e.components.iter_mut().for_each(fix);
    }
    scene.singletons.iter_mut().for_each(fix);
    for (path, s) in scene.schemas.iter_mut() {
        *s = live[path].clone();
    }
    Ok(warnings)
}
