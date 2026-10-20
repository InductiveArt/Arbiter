use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use arbiter::graph::{graph, integrity, judge};
use arbiter::pack::{Def, Edge, Finding, Suppression};
use arbiter::units::Repo;
use arbiter::{ledger, pack, units, verify};

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if let Some(command @ ("verify" | "fix")) = arguments.first().map(String::as_str) {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Ok(root) = PathBuf::from(&asked).canonicalize() else {
            eprintln!("{command}: no folder at {asked}");
            return ExitCode::from(2);
        };
        let packs = pack::all();
        let repo = units::scan(&root, &packs);
        let (outcomes, notes) = if command == "fix" {
            verify::fix(&root, &repo, &packs)
        } else {
            verify::verify(&root, &repo, &packs)
        };
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
            println!("{command}: {note}");
        }
        if outcomes.is_empty() {
            println!("{command}: no unit declares any tools, so nothing was judged");
            return ExitCode::from(2);
        }
        let failed = outcomes.iter().filter(|outcome| !outcome.passed).count();
        if failed == 0 {
            println!("{command}: {} checks passed", outcomes.len());
            return ExitCode::SUCCESS;
        }
        println!("{command}: {failed} of {} checks failed", outcomes.len());
        return ExitCode::FAILURE;
    }
    if arguments.first().map(String::as_str) == Some("blast") {
        let Some(name) = arguments.get(1) else {
            eprintln!("blast: name the symbol to look up");
            return ExitCode::from(2);
        };
        let asked = arguments.get(2).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("blast: no folder at {asked}");
            return ExitCode::from(2);
        };
        blast(name, &found.defs, &found.edges);
        return ExitCode::SUCCESS;
    }
    if arguments.first().map(String::as_str) == Some("findings") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("findings: no folder at {asked}");
            return ExitCode::from(2);
        };
        let mut all = judge(&found.defs, &found.edges, &found.repo);
        all.extend(integrity(&found));
        all.extend(ledger::blanket_findings(&found));
        all.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
        emit(&all, &arguments[2.min(arguments.len())..]);
        return ExitCode::SUCCESS;
    }
    if arguments.first().map(String::as_str) == Some("ledger") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("ledger: no folder at {asked}");
            return ExitCode::from(2);
        };
        let counted = ledger::counted(&found);
        let plural = if counted.len() == 1 { "" } else { "s" };
        println!("ledger: {} suppression{plural}", counted.len());
        for suppression in &counted {
            println!("  {}", shown(suppression));
        }
        match ledger::reference(&found.root) {
            None => println!("ledger: no reference recorded; arbiter accept records today's"),
            Some(reference) => {
                let extra = ledger::added(&counted, &reference);
                if extra.is_empty() {
                    println!("ledger: {} recorded, none added", reference.len());
                } else {
                    println!(
                        "ledger: {} recorded, {} added:",
                        reference.len(),
                        extra.len()
                    );
                    for suppression in &extra {
                        println!("  {}", shown(suppression));
                    }
                }
            }
        }
        let blankets = ledger::blankets(&found);
        if blankets.is_empty() {
            return ExitCode::SUCCESS;
        }
        let plural = if blankets.len() == 1 { "" } else { "s" };
        println!(
            "ledger: {} blanket suppression{plural}, quieting more than one named rule:",
            blankets.len()
        );
        for suppression in &blankets {
            println!("  {}", shown(suppression));
        }
        return ExitCode::FAILURE;
    }
    if arguments.first().map(String::as_str) == Some("accept") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("accept: no folder at {asked}");
            return ExitCode::from(2);
        };
        let blankets = ledger::blankets(&found);
        if !blankets.is_empty() {
            eprintln!(
                "accept: {} blanket suppressions are findings, never records",
                blankets.len()
            );
            for suppression in &blankets {
                eprintln!("  {}", shown(suppression));
            }
            return ExitCode::FAILURE;
        }
        let counted = ledger::counted(&found);
        match ledger::record(&found.root, &counted) {
            Ok(path) => {
                println!(
                    "accept: {} suppressions recorded in {}",
                    counted.len(),
                    path.to_string_lossy()
                );
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("accept: the reference could not be written: {error}");
                return ExitCode::from(2);
            }
        }
    }
    if arguments.first().map(String::as_str) == Some("integrity") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("integrity: no folder at {asked}");
            return ExitCode::from(2);
        };
        let findings = integrity(&found);
        if findings.is_empty() {
            println!("integrity: every unit stands on its own");
            return ExitCode::SUCCESS;
        }
        let plural = if findings.len() == 1 { "" } else { "s" };
        println!("integrity: {} finding{plural}:", findings.len());
        for finding in &findings {
            println!("  {} {}", place(finding), finding.reason);
        }
        return ExitCode::SUCCESS;
    }
    let asked = match arguments.first().map(String::as_str) {
        Some("economy") => arguments.get(1).cloned().unwrap_or_else(|| ".".to_string()),
        Some(folder) => folder.to_string(),
        None => ".".to_string(),
    };
    let Some(found) = graph(&asked) else {
        eprintln!("economy: no folder at {asked}");
        return ExitCode::from(2);
    };
    let findings = judge(&found.defs, &found.edges, &found.repo);
    report(&found.repo, &found.defs, &found.edges, &findings);
    ExitCode::SUCCESS
}

fn shown(suppression: &Suppression) -> String {
    format!(
        "{}:{} {}",
        suppression.file.to_string_lossy(),
        suppression.line,
        suppression.text
    )
}

// The file alone where no single line holds the finding.
fn place(finding: &Finding) -> String {
    if finding.line == 0 {
        finding.file.to_string_lossy().to_string()
    } else {
        format!("{}:{}", finding.file.to_string_lossy(), finding.line)
    }
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
            "  {} {} has {}",
            place(finding),
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
                "check": finding.check,
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
