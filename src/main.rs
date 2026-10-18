mod cpp;
mod python;
mod trees;
mod typescript;
mod units;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use units::{Lang, Repo};

pub struct Def {
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub unit: usize,
    pub lang: Lang,
    // Whether the defining file itself names the symbol away from its definition line.
    pub self_used: bool,
}

pub struct Edge {
    pub def: usize,
    pub file: PathBuf,
    pub unit: usize,
}

struct Finding {
    file: PathBuf,
    line: usize,
    name: String,
    reason: &'static str,
}

fn main() -> ExitCode {
    let asked = env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let Ok(root) = PathBuf::from(&asked).canonicalize() else {
        eprintln!("economy: no folder at {asked}");
        return ExitCode::from(2);
    };
    let repo = units::scan(&root);
    let mut defs: Vec<Def> = Vec::new();
    let mut python_imports: Vec<(PathBuf, String, String)> = Vec::new();
    let mut typescript_imports: Vec<(PathBuf, String, String)> = Vec::new();
    let mut cpp_sources: Vec<cpp::Source> = Vec::new();
    for (file, lang) in &repo.files {
        let Ok(source) = fs::read_to_string(root.join(file)) else {
            continue;
        };
        let unit = repo.unit_of(file);
        // Tests and a binary's entry are leaves of the graph, offering nothing for others to use.
        let surface =
            !repo.is_test(unit, file) && !repo.units[unit].root_files.contains(file);
        match lang {
            Lang::Python => {
                let Some(tree) = trees::parse(&python::language(), &source) else {
                    continue;
                };
                if surface {
                    let mentions = python::mentions(&tree, &source);
                    collect(&mut defs, python::definitions(&tree, &source), &mentions, file, unit, *lang);
                }
                for (module, name) in python::imports(&tree, &source) {
                    python_imports.push((file.clone(), module, name));
                }
            }
            Lang::TypeScript => {
                let Some(tree) = trees::parse(&typescript::language(), &source) else {
                    continue;
                };
                if surface {
                    let mentions = typescript::mentions(&tree, &source);
                    collect(&mut defs, typescript::definitions(&tree, &source), &mentions, file, unit, *lang);
                }
                for (from, name) in typescript::imports(&tree, &source) {
                    typescript_imports.push((file.clone(), from, name));
                }
            }
            Lang::Cpp => {
                let Some(tree) = trees::parse(&cpp::language(), &source) else {
                    continue;
                };
                if surface && units::is_header(file) {
                    collect(&mut defs, cpp::definitions(&tree, &source), &[], file, unit, *lang);
                }
                cpp_sources.push(cpp::read(file, &tree, &source));
            }
        }
    }
    let mut edges = python::resolve(&python_imports, &defs, &repo);
    edges.extend(typescript::resolve(&typescript_imports, &defs, &repo));
    edges.extend(cpp::resolve(&cpp_sources, &defs, &repo));
    let findings = judge(&defs, &edges, &repo);
    report(&repo, &defs, &edges, &findings);
    ExitCode::SUCCESS
}

fn collect(
    defs: &mut Vec<Def>,
    found: Vec<(String, usize)>,
    mentions: &[(String, usize)],
    file: &PathBuf,
    unit: usize,
    lang: Lang,
) {
    for (name, line) in found {
        let self_used = mentions
            .iter()
            .any(|(mentioned, at)| *mentioned == name && *at != line);
        defs.push(Def {
            name,
            file: file.clone(),
            line,
            unit,
            lang,
            self_used,
        });
    }
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
