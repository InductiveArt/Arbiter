use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::graph;
use crate::ledger;
use crate::pack;
use crate::pack::{Finding, Pack};
use crate::units;
use crate::units::Repo;

pub struct Outcome {
    pub unit: String,
    pub pack: &'static str,
    pub check: String,
    pub passed: bool,
    pub seconds: f64,
    pub tail: String,
}

// One whole judgment of a repository: both planes, with the ledger's footer.
pub struct Run {
    pub root: PathBuf,
    pub units: usize,
    pub files: usize,
    pub outcomes: Vec<Outcome>,
    pub notes: Vec<String>,
    pub findings: Vec<Finding>,
    pub counted: usize,
    // Suppressions added since the reference; None when none was recorded yet.
    pub added: Option<usize>,
}

const TAIL: usize = 15;

// Judges everything: the units' tools, then the graph. A fixing run repairs first,
// so the graph is read only after the repairs have settled.
pub fn run(asked: &str, fixing: bool) -> Option<Run> {
    let (outcomes, notes, found) = if fixing {
        let root = PathBuf::from(asked).canonicalize().ok()?;
        let packs = pack::all();
        let repo = units::scan(&root, &packs);
        let (outcomes, notes) = judged(&root, &repo, &packs, true);
        (outcomes, notes, graph::graph(asked)?)
    } else {
        let found = graph::graph(asked)?;
        let (outcomes, notes) = judged(&found.root, &found.repo, &found.packs, false);
        (outcomes, notes, found)
    };
    let mut findings = graph::judge(&found.defs, &found.edges, &found.repo);
    findings.extend(graph::integrity(&found));
    findings.extend(ledger::blanket_findings(&found));
    findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    let counted = ledger::counted(&found);
    let added =
        ledger::reference(&found.root).map(|reference| ledger::added(&counted, &reference).len());
    let counted = counted.len();
    Some(Run {
        root: found.root,
        units: found.repo.units.len(),
        files: found.repo.files.len(),
        outcomes,
        notes,
        findings,
        counted,
        added,
    })
}

// Per check: how many units passed, how many failed, and the seconds they took.
pub fn tallied(outcomes: &[Outcome]) -> BTreeMap<String, (usize, usize, f64)> {
    let mut tallies: BTreeMap<String, (usize, usize, f64)> = BTreeMap::new();
    for outcome in outcomes {
        let entry = tallies.entry(outcome.check.clone()).or_insert((0, 0, 0.0));
        if outcome.passed {
            entry.0 += 1;
        } else {
            entry.1 += 1;
        }
        entry.2 += outcome.seconds;
    }
    tallies
}

// Writes the run's detail where the dashboard points: a report of sections to read
// one at a time, and the same material as JSON lines. The folder never commits.
pub fn written(run: &Run) -> Option<PathBuf> {
    let folder = run.root.join(".arbiter").join("last");
    std::fs::create_dir_all(&folder).ok()?;
    std::fs::write(folder.join(".gitignore"), "*\n").ok()?;
    let mut report = String::from("# arbiter\n");
    for outcome in &run.outcomes {
        if outcome.passed {
            continue;
        }
        report.push_str(&format!("\n## {} — {}\n\n", outcome.check, outcome.unit));
        for line in outcome.tail.lines() {
            report.push_str(line);
            report.push('\n');
        }
    }
    let mut by_file: BTreeMap<&PathBuf, Vec<&Finding>> = BTreeMap::new();
    for finding in &run.findings {
        by_file.entry(&finding.file).or_default().push(finding);
    }
    for (file, findings) in &by_file {
        report.push_str(&format!("\n## {}\n\n", file.to_string_lossy()));
        for finding in findings {
            report.push_str(&format!(
                "{} {} {}\n",
                finding.line, finding.name, finding.reason
            ));
        }
    }
    std::fs::write(folder.join("report.md"), report).ok()?;
    let mut lines = String::new();
    for finding in &run.findings {
        lines.push_str(
            &serde_json::json!({
                "kind": "finding",
                "check": finding.check,
                "file": finding.file.to_string_lossy(),
                "line": finding.line,
                "symbol": finding.name,
                "reason": finding.reason,
            })
            .to_string(),
        );
        lines.push('\n');
    }
    for outcome in &run.outcomes {
        let mut entry = serde_json::json!({
            "kind": "outcome",
            "check": outcome.check,
            "unit": outcome.unit,
            "pack": outcome.pack,
            "passed": outcome.passed,
            "seconds": outcome.seconds,
        });
        if !outcome.passed {
            entry["tail"] = serde_json::Value::String(outcome.tail.clone());
        }
        lines.push_str(&entry.to_string());
        lines.push('\n');
    }
    std::fs::write(folder.join("findings.jsonl"), lines).ok()?;
    Some(folder.join("report.md"))
}

fn judged(
    root: &Path,
    repo: &Repo,
    packs: &[Box<dyn Pack>],
    fixing: bool,
) -> (Vec<Outcome>, Vec<String>) {
    let mut outcomes = Vec::new();
    let mut notes = Vec::new();
    for (index, unit) in repo.units.iter().enumerate() {
        // A unit on a test path is a fixture: data for some test, not code to judge.
        if repo.is_test(index, &unit.path) {
            continue;
        }
        let folder = root.join(&unit.path);
        let shown = if unit.path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            unit.path.to_string_lossy().to_string()
        };
        for pack in packs {
            let Some(manifest) = pack.manifest() else {
                continue;
            };
            let Ok(declared) = std::fs::read_to_string(folder.join(manifest)) else {
                continue;
            };
            if pack.container(&declared) {
                notes.push(format!(
                    "{shown} is a {} workspace; its members are the units judged",
                    pack.label()
                ));
                continue;
            }
            let started = Instant::now();
            if let Some(findings) = pack.wiring(&folder) {
                outcomes.push(Outcome {
                    unit: shown.clone(),
                    pack: pack.label(),
                    check: "wiring".to_string(),
                    passed: findings.is_empty(),
                    seconds: started.elapsed().as_secs_f64(),
                    tail: findings.join("\n"),
                });
            }
            let Some(adapter) = pack.adapter() else {
                notes.push(format!(
                    "{shown} is {}, which declares no tools yet",
                    pack.label()
                ));
                continue;
            };
            let home = materialized(pack.as_ref());
            for check in checks(adapter) {
                let check = Check {
                    name: check.name,
                    run: filled(&check.run, home.as_deref()),
                    fix: check.fix.map(|fix| filled(&fix, home.as_deref())),
                };
                if fixing {
                    if let Some(fix) = &check.fix {
                        let repair = Check {
                            name: check.name.clone(),
                            run: fix.clone(),
                            fix: None,
                        };
                        let outcome = executed(&shown, pack.label(), &folder, &repair);
                        if !outcome.passed {
                            outcomes.push(outcome);
                            continue;
                        }
                    }
                }
                outcomes.push(executed(&shown, pack.label(), &folder, &check));
            }
        }
        for pack in packs {
            for drift in pack.drift(root, &folder, fixing, &mut notes) {
                outcomes.push(Outcome {
                    unit: shown.clone(),
                    pack: pack.label(),
                    check: drift.check,
                    passed: drift.problems.is_empty(),
                    seconds: drift.seconds,
                    tail: drift.problems.join("\n"),
                });
            }
        }
    }
    (outcomes, notes)
}

struct Check {
    name: String,
    run: Vec<String>,
    fix: Option<Vec<String>>,
}

// The pack's configuration files, written outside the judged repository so a run
// changes nothing in it, at a place {pack} in a command resolves to.
fn materialized(pack: &dyn Pack) -> Option<std::path::PathBuf> {
    let configs = pack.configs();
    if configs.is_empty() {
        return None;
    }
    let folder = std::env::temp_dir()
        .join("arbiter-packs")
        .join(pack.label());
    std::fs::create_dir_all(&folder).ok()?;
    for (name, content) in configs {
        std::fs::write(folder.join(name), content).ok()?;
    }
    Some(folder)
}

fn filled(run: &[String], home: Option<&Path>) -> Vec<String> {
    let Some(home) = home else {
        return run.to_vec();
    };
    let shown = home.to_string_lossy();
    run.iter()
        .map(|word| word.replace("{pack}", &shown))
        .collect()
}

fn checks(adapter: &str) -> Vec<Check> {
    let Ok(parsed) = adapter.parse::<toml::Value>() else {
        return Vec::new();
    };
    let Some(entries) = parsed.get("check").and_then(toml::Value::as_array) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?.to_string();
            let run = words(entry.get("run")?)?;
            let fix = entry.get("fix").and_then(words);
            (!run.is_empty()).then_some(Check { name, run, fix })
        })
        .collect()
}

fn words(list: &toml::Value) -> Option<Vec<String>> {
    Some(
        list.as_array()?
            .iter()
            .filter_map(|word| word.as_str().map(str::to_string))
            .collect(),
    )
}

fn executed(unit: &str, pack: &'static str, folder: &Path, check: &Check) -> Outcome {
    let started = Instant::now();
    let finished = Command::new(&check.run[0])
        .args(&check.run[1..])
        .current_dir(folder)
        .output();
    let seconds = started.elapsed().as_secs_f64();
    let (passed, tail) = match finished {
        Ok(output) => (output.status.success(), last_lines(&output)),
        Err(error) => (false, format!("{} did not start: {error}", check.run[0])),
    };
    Outcome {
        unit: unit.to_string(),
        pack,
        check: check.name.clone(),
        passed,
        seconds,
        tail: if passed { String::new() } else { tail },
    }
}

fn last_lines(output: &std::process::Output) -> String {
    let whole = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<&str> = whole.lines().collect();
    let kept = &lines[lines.len().saturating_sub(TAIL)..];
    kept.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{tallied, Outcome};

    fn outcome(check: &str, passed: bool, seconds: f64) -> Outcome {
        Outcome {
            unit: "app".to_string(),
            pack: "rust",
            check: check.to_string(),
            passed,
            seconds,
            tail: String::new(),
        }
    }

    #[test]
    fn a_tally_counts_passes_and_failures_per_check_with_their_time() {
        let outcomes = [
            outcome("lint", true, 1.0),
            outcome("lint", false, 2.0),
            outcome("test", true, 3.0),
        ];
        let tallies = tallied(&outcomes);
        assert_eq!(tallies["lint"], (1, 1, 3.0));
        assert_eq!(tallies["test"], (1, 0, 3.0));
    }
}
