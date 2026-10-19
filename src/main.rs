use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use arbiter::graph::{graph, judge, Finding};
use arbiter::pack::{Def, Edge};
use arbiter::units::Repo;
use arbiter::{pack, units, verify};

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("verify") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Ok(root) = PathBuf::from(&asked).canonicalize() else {
            eprintln!("verify: no folder at {asked}");
            return ExitCode::from(2);
        };
        let packs = pack::all();
        let repo = units::scan(&root, &packs);
        let (outcomes, notes) = verify::verify(&root, &repo, &packs);
        for outcome in &outcomes {
            let answer = if outcome.passed { "ok    " } else { "failed" };
            println!(
                "  {}/{} {:10} {answer} {:.1}s",
                outcome.unit, outcome.pack, outcome.check, outcome.seconds
            );
            if !outcome.passed && !outcome.tail.is_empty() {
                for line in outcome.tail.lines() {
                    println!("    {line}");
                }
            }
        }
        for note in &notes {
            println!("verify: {note}");
        }
        if outcomes.is_empty() {
            println!("verify: no unit declares any tools, so nothing was judged");
            return ExitCode::from(2);
        }
        let failed = outcomes.iter().filter(|outcome| !outcome.passed).count();
        if failed == 0 {
            println!("verify: {} checks passed", outcomes.len());
            return ExitCode::SUCCESS;
        }
        println!("verify: {failed} of {} checks failed", outcomes.len());
        return ExitCode::FAILURE;
    }
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
        emit(
            &judge(&defs, &edges, &repo),
            &arguments[2.min(arguments.len())..],
        );
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
            .map(|edge| format!("  used by {}:{}", edge.file.to_string_lossy(), edge.line))
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
