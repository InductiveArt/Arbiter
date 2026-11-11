use std::fs;
use std::path::Path;
use std::process::ExitCode;

use crate::core::pack::Def;
use crate::core::pack::Edge;
use crate::core::task::Task;
use crate::laws::findings::drawn;
use crate::packs::graphed;

// The resolved symbol graph, questioned: what one unit stands on, and every
// use of a name.
pub fn tasks() -> Vec<Task> {
    vec![
        Task {
            name: "deps",
            line: "what one unit stands on, declared against resolved; bare, the whole graph",
            run: deps_query,
        },
        Task {
            name: "fallout",
            line: "every definition of a name and each resolved use",
            run: fallout_query,
        },
    ]
}

fn deps_query(rest: &[String]) -> ExitCode {
    let Some(name) = rest.first() else {
        let Some(found) = graphed(".") else {
            eprintln!("find: no folder at .");
            return ExitCode::from(2);
        };
        return drawn_graph(&found);
    };
    let asked = rest.get(1).cloned().unwrap_or_else(|| ".".to_string());
    let Some(found) = graphed(&asked) else {
        eprintln!("find: no folder at {asked}");
        return ExitCode::from(2);
    };
    deps(name, &found)
}

fn fallout_query(rest: &[String]) -> ExitCode {
    let Some(name) = rest.first() else {
        eprintln!("find fallout: name the symbol to look up");
        return ExitCode::from(2);
    };
    let asked = rest.get(1).cloned().unwrap_or_else(|| ".".to_string());
    let Some(found) = graphed(&asked) else {
        eprintln!("find: no folder at {asked}");
        return ExitCode::from(2);
    };
    fallout(name, &found.defs, &found.edges);
    ExitCode::SUCCESS
}

// One unit's whole dependency picture, every language in the one format: the
// written edges beside the observed ones, and the third parties its locks name.
fn deps(name: &str, found: &crate::core::graph::Graph) -> ExitCode {
    let shown_unit = |unit: usize| {
        let path = &found.repo.units[unit].path;
        if path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            path.to_string_lossy().to_string()
        }
    };
    let Some(unit) = (0..found.repo.units.len()).find(|&unit| shown_unit(unit) == *name) else {
        return scoped(name, found);
    };
    let mut uses = std::collections::BTreeSet::new();
    let mut users = std::collections::BTreeSet::new();
    for edge in &found.edges {
        let owner = found.defs[edge.def].unit;
        if owner == edge.unit {
            continue;
        }
        if edge.unit == unit {
            uses.insert(shown_unit(owner));
        }
        if owner == unit {
            users.insert(shown_unit(edge.unit));
        }
    }
    println!("deps: {name}");
    let listed = |set: &std::collections::BTreeSet<String>| {
        if set.is_empty() {
            "nothing".to_string()
        } else {
            set.iter().cloned().collect::<Vec<_>>().join(", ")
        }
    };
    println!("  uses: {}", listed(&uses));
    println!("  used by: {}", listed(&users));
    let declared = fs::read_to_string(found.root.join(".arbiter/units.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Value>().ok());
    let entry = declared
        .as_ref()
        .and_then(|parsed| parsed.get("units"))
        .and_then(|units| units.get(name));
    let names = |value: Option<&toml::Value>| -> Option<Vec<String>> {
        Some(
            value?
                .as_array()?
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect(),
        )
    };
    match names(entry.and_then(|entry| entry.get("deps"))) {
        Some(deps) => println!("  declares: {}", deps.join(", ")),
        None => println!("  declares: nothing"),
    }
    match names(entry.and_then(|entry| entry.get("visibility"))) {
        Some(grants) => println!("  grants: {}", grants.join(", ")),
        None => println!("  grants: everyone; no visibility is declared"),
    }
    for pack in &found.packs {
        let locks = pack.lockfiles();
        if locks.is_empty() {
            continue;
        }
        for path in &found.repo.lockfiles {
            let Some(name) = path.file_name().map(|name| name.to_string_lossy()) else {
                continue;
            };
            if !locks.contains(&name.as_ref()) {
                continue;
            }
            if found.repo.unit_of(path) != unit {
                continue;
            }
            let Ok(text) = fs::read_to_string(found.root.join(path)) else {
                continue;
            };
            let locked = pack.locked(&name, &text);
            println!(
                "  third parties ({}): {} locked",
                pack.label(),
                locked.len()
            );
            for (name, version) in locked {
                println!("    {name} {version}");
            }
        }
    }
    ExitCode::SUCCESS
}

// A folder or file inside a unit: the same read at its own grain — each file
// it reaches beyond itself, and each outside file standing on it.
fn scoped(asked: &str, found: &crate::core::graph::Graph) -> ExitCode {
    let scope = Path::new(asked.trim_start_matches("./"))
        .components()
        .collect::<std::path::PathBuf>();
    if scope.as_os_str().is_empty() || !found.root.join(&scope).exists() {
        eprintln!("deps: no unit or path named {asked}");
        return ExitCode::from(2);
    }
    let within = |file: &Path| file.starts_with(&scope);
    let mut uses = std::collections::BTreeSet::new();
    let mut users = std::collections::BTreeSet::new();
    for edge in &found.edges {
        let owner = &found.defs[edge.def].file;
        if within(&edge.file) && !within(owner) {
            uses.insert(owner.to_string_lossy().to_string());
        }
        if within(owner) && !within(&edge.file) {
            users.insert(edge.file.to_string_lossy().to_string());
        }
    }
    println!("deps: {}", scope.display());
    for (title, set) in [("uses", &uses), ("used by", &users)] {
        if set.is_empty() {
            println!("  {title}: nothing");
            continue;
        }
        println!("  {title}:");
        for file in set {
            println!("    {file}");
        }
    }
    ExitCode::SUCCESS
}

// The whole resolved unit graph as one mermaid fence, declared against real:
// an edge the declarations refuse is drawn dashed and named undeclared. A
// measure, never findings, so the fence pastes into any page the host draws.
fn drawn_graph(found: &crate::core::graph::Graph) -> ExitCode {
    let edges = drawn(found);
    let mut ids: std::collections::BTreeMap<&str, usize> = Default::default();
    for (user, owner, _) in &edges {
        let next = ids.len();
        ids.entry(user.as_str()).or_insert(next);
        let next = ids.len();
        ids.entry(owner.as_str()).or_insert(next);
    }
    println!("```mermaid");
    println!("flowchart TD");
    for (name, id) in &ids {
        println!("  u{id}[\"{name}\"]");
    }
    for (user, owner, breaks) in &edges {
        let from = ids[user.as_str()];
        let to = ids[owner.as_str()];
        if *breaks {
            println!("  u{from} -. undeclared .-> u{to}");
        } else {
            println!("  u{from} --> u{to}");
        }
    }
    if edges.is_empty() {
        println!("  %% no cross-unit edges resolved");
    }
    println!("  %% test units are not drawn");
    println!("```");
    ExitCode::SUCCESS
}

fn fallout(name: &str, defs: &[Def], edges: &[Edge]) {
    let matched: Vec<(usize, &Def)> = defs
        .iter()
        .enumerate()
        .filter(|(_, def)| def.name == name)
        .collect();
    if matched.is_empty() {
        println!("fallout: no public symbol named {name}");
        return;
    }
    for (index, def) in matched {
        println!("{}:{} {}", def.file.to_string_lossy(), def.line, def.name);
        let mut users: Vec<String> = edges
            .iter()
            .filter(|edge| edge.def == index && edge.file != def.file)
            .map(|edge| format!("  used by {}:{}", edge.file.to_string_lossy(), edge.line))
            .collect();
        users.sort();
        users.dedup();
        if def.self_used {
            println!("  used by its own file");
        } else if users.is_empty() {
            println!("  used by nothing");
        }
        for user in users {
            println!("{user}");
        }
    }
}
