mod cpp;
mod pack;
mod python;
mod rust;
mod trees;
mod typescript;
mod units;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use pack::{Def, Edge};
use units::Repo;

struct Finding {
    file: PathBuf,
    line: usize,
    name: String,
    reason: &'static str,
}

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("blast") {
        let Some(name) = arguments.get(1) else {
            eprintln!("blast: name the symbol to look up");
            return ExitCode::from(2);
        };
        let asked = arguments.get(2).cloned().unwrap_or_else(|| ".".to_string());
        let Some((_, defs, edges)) = graph(&asked) else {
            eprintln!("blast: no folder at {asked}");
            return ExitCode::from(2);
        };
        blast(name, &defs, &edges);
        return ExitCode::SUCCESS;
    }
    if arguments.first().map(String::as_str) == Some("findings") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some((repo, defs, edges)) = graph(&asked) else {
            eprintln!("findings: no folder at {asked}");
            return ExitCode::from(2);
        };
        emit(&judge(&defs, &edges, &repo), &arguments[2.min(arguments.len())..]);
        return ExitCode::SUCCESS;
    }
    let asked = match arguments.first().map(String::as_str) {
        Some("economy") => arguments.get(1).cloned().unwrap_or_else(|| ".".to_string()),
        Some(folder) => folder.to_string(),
        None => ".".to_string(),
    };
    let Some((repo, defs, edges)) = graph(&asked) else {
        eprintln!("economy: no folder at {asked}");
        return ExitCode::from(2);
    };
    let findings = judge(&defs, &edges, &repo);
    report(&repo, &defs, &edges, &findings);
    ExitCode::SUCCESS
}

fn graph(asked: &str) -> Option<(Repo, Vec<Def>, Vec<Edge>)> {
    let root = PathBuf::from(asked).canonicalize().ok()?;
    let mut packs = pack::all();
    let mut repo = units::scan(&root, &packs);
    let rooted: Vec<PathBuf> = packs
        .iter()
        .flat_map(|pack| pack.roots(&root, &repo))
        .collect();
    for file in rooted {
        let unit = repo.unit_of(&file);
        repo.units[unit].root_files.insert(file);
    }
    let mut defs: Vec<Def> = Vec::new();
    for file in &repo.files {
        let Ok(source) = fs::read_to_string(root.join(file)) else {
            continue;
        };
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let Some(index) = packs.iter().position(|pack| pack.handles(&name)) else {
            continue;
        };
        let unit = repo.unit_of(file);
        // Tests and a root are leaves of the graph, offering nothing for others to use.
        let surface =
            !repo.is_test(unit, file) && !repo.units[unit].root_files.contains(file);
        for definition in packs[index].scan(file, &source, surface) {
            defs.push(Def {
                name: definition.name,
                file: file.clone(),
                line: definition.line,
                unit,
                pack: index,
                self_used: definition.self_used,
            });
        }
    }
    let mut edges = Vec::new();
    for (index, pack) in packs.iter().enumerate() {
        edges.extend(pack.resolve(index, &defs, &repo));
    }
    Some((repo, defs, edges))
}

fn judge(defs: &[Def], edges: &[Edge], repo: &Repo) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (index, def) in defs.iter().enumerate() {
        if def.self_used {
            continue;
        }
        let users: Vec<&Edge> = edges
            .iter()
            .filter(|edge| edge.def == index && edge.file != def.file)
            .collect();
        if users.iter().any(|edge| edge.unit != def.unit) {
            continue;
        }
        let used_inside = users
            .iter()
            .any(|edge| !repo.is_test(edge.unit, &edge.file));
        if used_inside {
            continue;
        }
        findings.push(Finding {
            file: def.file.clone(),
            line: def.line,
            name: def.name.clone(),
            reason: if users.is_empty() {
                "no user anywhere"
            } else {
                "no user outside its own tests"
            },
        });
    }
    findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    findings
}

fn report(repo: &Repo, defs: &[Def], edges: &[Edge], findings: &[Finding]) {
    println!(
        "economy: {} units, {} files, {} public symbols, {} resolved uses",
        repo.units.len(),
        repo.files.len(),
        defs.len(),
        edges.len()
    );
    if findings.is_empty() {
        println!("economy: every public symbol has a user outside its unit");
        return;
    }
    let plural = if findings.len() == 1 { "" } else { "s" };
    println!(
        "economy: {} public symbol{plural} inviting deletion:",
        findings.len()
    );
    for finding in findings {
        println!(
            "  {}:{} {} has {}",
            finding.file.to_string_lossy(),
            finding.line,
            finding.name,
            finding.reason
        );
    }
}

// One JSON object per line, held to the named files when any are named.
fn emit(findings: &[Finding], files: &[String]) {
    for finding in findings {
        let shown = finding.file.to_string_lossy();
        if !files.is_empty() && !files.iter().any(|file| *file == shown) {
            continue;
        }
        println!(
            "{}",
            serde_json::json!({
                "check": "economy",
                "file": shown,
                "line": finding.line,
                "symbol": finding.name,
                "reason": finding.reason,
            })
        );
    }
}

fn blast(name: &str, defs: &[Def], edges: &[Edge]) {
    let matched: Vec<(usize, &Def)> = defs
        .iter()
        .enumerate()
        .filter(|(_, def)| def.name == name)
        .collect();
    if matched.is_empty() {
        println!("blast: no public symbol named {name}");
        return;
    }
    for (index, def) in matched {
        println!("{}:{} {}", def.file.to_string_lossy(), def.line, def.name);
        let mut users: Vec<String> = edges
            .iter()
            .filter(|edge| edge.def == index && edge.file != def.file)
            .map(|edge| format!("  used by {}", edge.file.to_string_lossy()))
            .collect();
        users.sort();
        users.dedup();
        if users.is_empty() {
            println!("  used by nothing");
        }
        for user in users {
            println!("{user}");
        }
    }
}
