//! The `sage-scene v1` and `sage-save v1` text formats.
//!
//! Line oriented and deterministic: the same scene always writes the same
//! bytes, in an order that does not depend on hashing, so files diff and
//! merge. Floats print as the shortest text that parses back to the same
//! bits; NaN and infinities print as their bits (`f32:7fc00000`).
//!
//! ```text
//! sage-save v1                      (or: sage-scene v1)
//! next-sid 62                       save only
//! rng 1a2b.. 3c4d.. 5e6f.. 7a8b..   save only, hex
//! module pong.play 1                save only
//! extra contacts "..."              save only
//! mode 2d
//! schema pong.Player 2 side:u8 points:u32
//! string 00ab.. "Hello"
//! entity 50 "left player"           the name is optional
//!   c pong.Player side=0 points=3
//!   tag pong.Ball
//!   pair pong.Opponent sid=51       or: pair flecs:IsA path=pong.Base
//! singleton sage.Clock tick=240
//! ```

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};

use crate::{
    Kind, Mode, Save, SaveMeta, Scene, SceneComponent, SceneEntity, ScenePair, Schemas,
    StoredSchema, StringTable, Value,
};

pub const SCENE_HEADER: &str = "sage-scene v1";
pub const SAVE_HEADER: &str = "sage-save v1";

/// A parse error, with the 1-based line it was found on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneError {
    pub line: usize,
    pub message: String,
}

impl SceneError {
    pub fn new(line: usize, message: impl Into<String>) -> SceneError {
        SceneError {
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for SceneError {}

pub enum Document {
    Scene(Scene),
    Save(Save),
}

/// FNV-1a over the UTF-8 bytes: the id of an interned string. The empty
/// string is 0, so a zeroed component means "no text".
pub fn hash_str(s: &str) -> u64 {
    if s.is_empty() {
        return 0;
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // 0 is reserved for the empty string.
    if h == 0 { 1 } else { h }
}

// ----- values ------------------------------------------------------------------

pub fn format_value(v: &Value) -> String {
    match *v {
        Value::Bool(b) => b.to_string(),
        Value::U8(x) => x.to_string(),
        Value::I32(x) => x.to_string(),
        Value::U32(x) => x.to_string(),
        Value::I64(x) => x.to_string(),
        Value::U64(x) | Value::Entity(x) => x.to_string(),
        Value::F32(x) if x.is_finite() => format!("{x:?}"),
        Value::F32(x) => format!("f32:{:08x}", x.to_bits()),
        Value::F64(x) if x.is_finite() => format!("{x:?}"),
        Value::F64(x) => format!("f64:{:016x}", x.to_bits()),
    }
}

pub fn parse_value(kind: Kind, s: &str) -> Result<Value, String> {
    let bad = |what: &str| format!("{s:?} is not a valid {what}");
    Ok(match kind {
        Kind::Bool => match s {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => return Err(bad("bool")),
        },
        Kind::U8 => Value::U8(s.parse().map_err(|_| bad("u8"))?),
        Kind::I32 => Value::I32(s.parse().map_err(|_| bad("i32"))?),
        Kind::U32 => Value::U32(s.parse().map_err(|_| bad("u32"))?),
        Kind::I64 => Value::I64(s.parse().map_err(|_| bad("i64"))?),
        Kind::U64 => Value::U64(s.parse().map_err(|_| bad("u64"))?),
        Kind::Entity => Value::Entity(s.parse().map_err(|_| bad("entity id"))?),
        Kind::F32 => match s.strip_prefix("f32:") {
            Some(bits) => Value::F32(f32::from_bits(
                u32::from_str_radix(bits, 16).map_err(|_| bad("f32 bit pattern"))?,
            )),
            None => {
                let f: f32 = s.parse().map_err(|_| bad("f32"))?;
                if !f.is_finite() {
                    return Err(format!("{s:?}: write non-finite floats as f32:<hex bits>"));
                }
                Value::F32(f)
            }
        },
        Kind::F64 => match s.strip_prefix("f64:") {
            Some(bits) => Value::F64(f64::from_bits(
                u64::from_str_radix(bits, 16).map_err(|_| bad("f64 bit pattern"))?,
            )),
            None => {
                let f: f64 = s.parse().map_err(|_| bad("f64"))?;
                if !f.is_finite() {
                    return Err(format!("{s:?}: write non-finite floats as f64:<hex bits>"));
                }
                Value::F64(f)
            }
        },
    })
}

// ----- quoting -----------------------------------------------------------------

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

struct Tok {
    text: String,
    quoted: bool,
}

/// Splits a line on whitespace; a `"..."` token may hold spaces and escapes.
fn tokens(line: &str) -> Result<Vec<Tok>, String> {
    let mut out = vec![];
    let mut chars = line.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            let mut s = String::new();
            loop {
                match chars.next() {
                    None => return Err("unterminated string".into()),
                    Some('"') => break,
                    Some('\\') => match chars.next() {
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some('n') => s.push('\n'),
                        Some('r') => s.push('\r'),
                        Some('t') => s.push('\t'),
                        Some('u') => {
                            if chars.next() != Some('{') {
                                return Err("expected { after \\u".into());
                            }
                            let mut hex = String::new();
                            loop {
                                match chars.next() {
                                    Some('}') => break,
                                    Some(h) => hex.push(h),
                                    None => return Err("unterminated \\u{".into()),
                                }
                            }
                            let cp = u32::from_str_radix(&hex, 16)
                                .ok()
                                .and_then(char::from_u32)
                                .ok_or_else(|| format!("bad escape \\u{{{hex}}}"))?;
                            s.push(cp);
                        }
                        other => return Err(format!("bad escape \\{}", other.unwrap_or(' '))),
                    },
                    Some(c) => s.push(c),
                }
            }
            out.push(Tok {
                text: s,
                quoted: true,
            });
        } else {
            let mut s = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                s.push(c);
                chars.next();
            }
            out.push(Tok {
                text: s,
                quoted: false,
            });
        }
    }
    Ok(out)
}

// ----- writing -----------------------------------------------------------------

fn write_component(out: &mut String, lead: &str, c: &SceneComponent) {
    let _ = write!(out, "{lead}{}", c.path);
    for (name, v) in &c.fields {
        let _ = write!(out, " {name}={}", format_value(v));
    }
    out.push('\n');
}

pub fn write_scene(scene: &Scene, meta: Option<&SaveMeta>) -> String {
    let mut out = String::new();
    out.push_str(if meta.is_some() {
        SAVE_HEADER
    } else {
        SCENE_HEADER
    });
    out.push('\n');
    if let Some(m) = meta {
        let _ = writeln!(out, "next-sid {}", m.next_sid);
        let _ = writeln!(
            out,
            "rng {:x} {:x} {:x} {:x}",
            m.rng[0], m.rng[1], m.rng[2], m.rng[3]
        );
        for (path, on) in &m.modules {
            let _ = writeln!(out, "module {path} {}", *on as u8);
        }
        for (k, v) in &m.extra {
            let _ = writeln!(out, "extra {k} {}", quote(v));
        }
    }
    let _ = writeln!(
        out,
        "mode {}",
        match scene.mode {
            Mode::TwoD => "2d",
            Mode::ThreeD => "3d",
        }
    );
    for (path, s) in &scene.schemas {
        let _ = write!(out, "schema {path} {}", s.version);
        for (name, kind) in &s.fields {
            let _ = write!(out, " {name}:{}", kind.name());
        }
        out.push('\n');
    }
    for (id, s) in scene.strings.iter() {
        let _ = writeln!(out, "string {id:016x} {}", quote(s));
    }
    for e in &scene.entities {
        let _ = write!(out, "entity {}", e.sid);
        if let Some(n) = &e.name {
            let _ = write!(out, " {}", quote(n));
        }
        out.push('\n');
        for c in &e.components {
            write_component(&mut out, "  c ", c);
        }
        for t in &e.tags {
            let _ = writeln!(out, "  tag {t}");
        }
        for p in &e.pairs {
            if p.target_sid != 0 {
                let _ = writeln!(out, "  pair {} sid={}", p.rel, p.target_sid);
            } else {
                let _ = writeln!(out, "  pair {} path={}", p.rel, p.target_path);
            }
        }
    }
    for s in &scene.singletons {
        write_component(&mut out, "singleton ", s);
    }
    out
}

// ----- parsing -----------------------------------------------------------------

/// Reads `name=value` tokens against the schema stored in the file.
fn read_fields(
    line: usize,
    schema: &StoredSchema,
    path: &str,
    toks: &[Tok],
) -> Result<Vec<(String, Value)>, SceneError> {
    let mut found: BTreeMap<&str, &str> = BTreeMap::new();
    for t in toks {
        let (k, v) = t
            .text
            .split_once('=')
            .filter(|_| !t.quoted)
            .ok_or_else(|| {
                SceneError::new(line, format!("expected name=value, found {:?}", t.text))
            })?;
        if found.insert(k, v).is_some() {
            return Err(SceneError::new(
                line,
                format!("{path}: field {k} given twice"),
            ));
        }
    }
    let mut out = vec![];
    for (name, kind) in &schema.fields {
        let raw = found
            .remove(name.as_str())
            .ok_or_else(|| SceneError::new(line, format!("{path}: missing field {name}")))?;
        let v = parse_value(*kind, raw)
            .map_err(|m| SceneError::new(line, format!("{path}.{name}: {m}")))?;
        out.push((name.clone(), v));
    }
    if let Some((k, _)) = found.into_iter().next() {
        return Err(SceneError::new(
            line,
            format!("{path}: the file's schema has no field {k}"),
        ));
    }
    Ok(out)
}

pub fn parse(src: &str) -> Result<Document, SceneError> {
    let mut lines = src.lines().enumerate().map(|(i, l)| (i + 1, l));
    let (_, first) = lines
        .next()
        .ok_or_else(|| SceneError::new(1, "empty file"))?;
    let is_save = match first.trim() {
        SCENE_HEADER => false,
        SAVE_HEADER => true,
        other => {
            return Err(SceneError::new(
                1,
                format!("expected `{SCENE_HEADER}` or `{SAVE_HEADER}`, found {other:?}"),
            ));
        }
    };
    let mut scene = Scene::new(Mode::TwoD);
    let mut meta = SaveMeta {
        next_sid: 0,
        rng: [0; 4],
        modules: vec![],
        extra: BTreeMap::new(),
    };
    let (mut saw_next_sid, mut saw_rng) = (false, false);
    let mut strings: Vec<(usize, u64, String)> = vec![];
    // Components are checked against the schema lines, which may come later
    // in a hand-edited file; read them first.
    let mut body: Vec<(usize, &str)> = vec![];
    for (n, raw) in lines {
        if raw.trim().is_empty() || raw.trim_start().starts_with('#') {
            continue;
        }
        body.push((n, raw));
    }
    for (n, raw) in &body {
        let t = tokens(raw).map_err(|m| SceneError::new(*n, m))?;
        if t.first().map(|t| t.text.as_str()) == Some("schema") {
            let path = t
                .get(1)
                .ok_or_else(|| SceneError::new(*n, "schema needs a path"))?;
            let version: u32 = t
                .get(2)
                .and_then(|v| v.text.parse().ok())
                .ok_or_else(|| SceneError::new(*n, "schema needs a version number"))?;
            let mut fields = vec![];
            for f in &t[3..] {
                let (name, kind) = f
                    .text
                    .split_once(':')
                    .and_then(|(a, b)| Kind::from_name(b).map(|k| (a, k)))
                    .ok_or_else(|| {
                        SceneError::new(*n, format!("expected name:kind, found {:?}", f.text))
                    })?;
                fields.push((name.to_string(), kind));
            }
            if scene
                .schemas
                .insert(path.text.clone(), StoredSchema { version, fields })
                .is_some()
            {
                return Err(SceneError::new(
                    *n,
                    format!("schema {} given twice", path.text),
                ));
            }
        }
    }
    for (n, raw) in &body {
        let n = *n;
        let indented = raw.starts_with(' ') || raw.starts_with('\t');
        let t = tokens(raw).map_err(|m| SceneError::new(n, m))?;
        let word = t[0].text.as_str();
        let need = |i: usize, what: &str| -> Result<&Tok, SceneError> {
            t.get(i)
                .ok_or_else(|| SceneError::new(n, format!("{word}: missing {what}")))
        };
        if indented {
            let e = scene
                .entities
                .last_mut()
                .ok_or_else(|| SceneError::new(n, format!("`{word}` outside an entity")))?;
            match word {
                "c" => {
                    let path = need(1, "component path")?.text.clone();
                    let schema = scene
                        .schemas
                        .get(&path)
                        .ok_or_else(|| SceneError::new(n, format!("no schema line for {path}")))?;
                    let fields = read_fields(n, schema, &path, &t[2..])?;
                    e.components.push(SceneComponent { path, fields });
                }
                "tag" => e.tags.push(need(1, "tag path")?.text.clone()),
                "pair" => {
                    let rel = need(1, "relationship")?.text.clone();
                    let target = &need(2, "sid=<n> or path=<p>")?.text;
                    let pair = if let Some(s) = target.strip_prefix("sid=") {
                        ScenePair {
                            rel,
                            target_sid: s
                                .parse()
                                .map_err(|_| SceneError::new(n, format!("bad sid {s:?}")))?,
                            target_path: String::new(),
                        }
                    } else if let Some(p) = target.strip_prefix("path=") {
                        ScenePair {
                            rel,
                            target_sid: 0,
                            target_path: p.to_string(),
                        }
                    } else {
                        return Err(SceneError::new(
                            n,
                            "pair target must be sid=<n> or path=<p>",
                        ));
                    };
                    e.pairs.push(pair);
                }
                _ => return Err(SceneError::new(n, format!("unknown entity line `{word}`"))),
            }
            continue;
        }
        match word {
            "schema" => {}
            "mode" => {
                scene.mode = match need(1, "2d or 3d")?.text.as_str() {
                    "2d" => Mode::TwoD,
                    "3d" => Mode::ThreeD,
                    other => return Err(SceneError::new(n, format!("unknown mode {other:?}"))),
                }
            }
            "string" => {
                let id = u64::from_str_radix(&need(1, "id")?.text, 16)
                    .map_err(|_| SceneError::new(n, "bad string id"))?;
                strings.push((n, id, need(2, "text")?.text.clone()));
            }
            "entity" => {
                let sid: u64 = need(1, "StableId")?
                    .text
                    .parse()
                    .map_err(|_| SceneError::new(n, "bad StableId"))?;
                if scene.entities.last().is_some_and(|p| p.sid >= sid) {
                    return Err(SceneError::new(
                        n,
                        format!("entity {sid} is out of order (entities are sorted by StableId)"),
                    ));
                }
                scene.entities.push(SceneEntity {
                    sid,
                    name: t.get(2).map(|n| n.text.clone()),
                    components: vec![],
                    tags: vec![],
                    pairs: vec![],
                });
            }
            "singleton" => {
                let path = need(1, "component path")?.text.clone();
                let schema = scene
                    .schemas
                    .get(&path)
                    .ok_or_else(|| SceneError::new(n, format!("no schema line for {path}")))?;
                let fields = read_fields(n, schema, &path, &t[2..])?;
                scene.singletons.push(SceneComponent { path, fields });
            }
            "next-sid" if is_save => {
                meta.next_sid = need(1, "number")?
                    .text
                    .parse()
                    .map_err(|_| SceneError::new(n, "bad number"))?;
                saw_next_sid = true;
            }
            "rng" if is_save => {
                for i in 0..4 {
                    meta.rng[i] = u64::from_str_radix(&need(1 + i, "four hex words")?.text, 16)
                        .map_err(|_| SceneError::new(n, "bad rng word"))?;
                }
                saw_rng = true;
            }
            "module" if is_save => {
                let path = need(1, "path")?.text.clone();
                let on = match need(2, "0 or 1")?.text.as_str() {
                    "0" => false,
                    "1" => true,
                    _ => return Err(SceneError::new(n, "module state must be 0 or 1")),
                };
                meta.modules.push((path, on));
            }
            "extra" if is_save => {
                let k = need(1, "key")?.text.clone();
                let v = need(2, "quoted value")?;
                if !v.quoted {
                    return Err(SceneError::new(n, "extra values must be quoted"));
                }
                if meta.extra.insert(k.clone(), v.text.clone()).is_some() {
                    return Err(SceneError::new(n, format!("extra {k} given twice")));
                }
            }
            other => return Err(SceneError::new(n, format!("unknown line `{other}`"))),
        }
    }
    let mut table = StringTable::new();
    for (n, id, s) in strings {
        if hash_str(&s) != id {
            return Err(SceneError::new(
                n,
                format!("string id {id:016x} is not the hash of its text (hand-edited?)"),
            ));
        }
        table.intern(&s);
    }
    scene.strings = table;
    if is_save {
        if !saw_next_sid || !saw_rng {
            return Err(SceneError::new(
                1,
                "a save game needs `next-sid` and `rng` lines",
            ));
        }
        Ok(Document::Save(Save { scene, meta }))
    } else {
        Ok(Document::Scene(scene))
    }
}

/// `(path, version, fields)`.
pub type SchemaSpec<'a> = (&'a str, u32, &'a [(&'a str, Kind)]);

/// Builds a [`Schemas`] from `(path, version, fields)`, for tests and for
/// hosts that describe native components by hand.
pub fn schemas_from(list: &[SchemaSpec]) -> Schemas {
    list.iter()
        .map(|(p, v, f)| {
            (
                p.to_string(),
                StoredSchema {
                    version: *v,
                    fields: f.iter().map(|(n, k)| (n.to_string(), *k)).collect(),
                },
            )
        })
        .collect()
}
