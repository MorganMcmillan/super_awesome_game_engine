//! Editor stubs (`.pyi`) for native components that scripts import by path.
//!
//! Generated from the same reflection data the binding uses at run time, so
//! a host can check its committed stubs against `Ecs::type_stubs` in a test.

use std::collections::{BTreeMap, BTreeSet};

use crate::registry::{Kind, Layout, Registry};

const ROOT: &str = "sage";

const HEADER: &str = "\
# Generated from the reflection data of the host's native components
# (sage_ecs_py `Ecs::type_stubs`). Editors read this; the engine never does.
# Do not edit by hand; regenerate it instead.
";

fn annotation(kind: Kind) -> &'static str {
    match kind {
        Kind::Bool => "bool",
        Kind::Entity => "Entity",
        other => other.name(),
    }
}

/// The stub file for `module`, relative to the package directory.
fn file_for(module: &str, modules: &BTreeSet<String>) -> String {
    if module == ROOT {
        return "_native.pyi".into();
    }
    let rel = module[ROOT.len() + 1..].replace('.', "/");
    let prefix = format!("{module}.");
    if modules.iter().any(|m| m.starts_with(&prefix)) {
        format!("{rel}/__init__.pyi")
    } else {
        format!("{rel}.pyi")
    }
}

pub fn generate(reg: &Registry, natives: &[u64]) -> Vec<(String, String)> {
    let types = reg.types.borrow();
    // module -> (class name -> class source)
    let mut modules: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for id in natives {
        let t = &types[id];
        let Some((module, name)) = t.path.rsplit_once('.') else {
            continue;
        };
        if module != ROOT && !module.starts_with(&format!("{ROOT}.")) {
            continue;
        }
        let fields = match &t.layout {
            Layout::Struct(fields) => fields.as_slice(),
            _ => &[],
        };
        let mut class = format!(
            "class {name}(Component):\n    \"\"\"Native component `{}`.\"\"\"\n\n",
            t.path
        );
        for f in fields {
            class += &format!("    {}: {}\n", f.name, annotation(f.kind));
        }
        let params: String = fields
            .iter()
            .map(|f| format!(", {}: {} = ...", f.name, annotation(f.kind)))
            .collect();
        class += &format!("    def __init__(self{params}) -> None: ...\n");
        modules
            .entry(module.to_string())
            .or_default()
            .insert(name.to_string(), class);
    }
    // Parent namespaces of nested modules need a file too.
    let names: BTreeSet<String> = modules.keys().cloned().collect();
    for m in &names {
        let mut cur = m.as_str();
        while let Some((parent, _)) = cur.rsplit_once('.') {
            modules.entry(parent.to_string()).or_default();
            cur = parent;
        }
    }
    let all: BTreeSet<String> = modules.keys().cloned().collect();

    let mut files = vec![];
    for (module, classes) in &modules {
        let body: Vec<&str> = classes.values().map(String::as_str).collect();
        let mut imports: BTreeSet<&str> = BTreeSet::new();
        for id in natives {
            let t = &types[id];
            if t.path.rsplit_once('.').map(|(m, _)| m) != Some(module.as_str()) {
                continue;
            }
            imports.insert("Component");
            if let Layout::Struct(fields) = &t.layout {
                for f in fields {
                    if !matches!(f.kind, Kind::Bool) {
                        imports.insert(annotation(f.kind));
                    }
                }
            }
        }
        let mut text = HEADER.to_string();
        if !imports.is_empty() {
            // Component and Entity first, then the field kinds.
            let mut ordered: Vec<&str> = ["Component", "Entity"]
                .into_iter()
                .filter(|n| imports.contains(n))
                .collect();
            ordered.extend(
                imports
                    .iter()
                    .filter(|n| !["Component", "Entity"].contains(n)),
            );
            text += &format!("\nfrom sage.ecs import {}\n", ordered.join(", "));
        }
        for class in body {
            text += "\n\n";
            text += class;
        }
        files.push((file_for(module, &all), text));
    }
    files.sort();
    files
}
