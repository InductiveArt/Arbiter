use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::core::graph;
use crate::core::pack::Finding;
use crate::core::units;
use crate::judge::findings;
use crate::judge::ledger;
use crate::judge::outcomes;
use crate::judge::outcomes::Outcome;
use crate::packs;

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
    pub third: usize,
    // Third parties added since the reference; None when none was recorded yet.
    pub third_added: Option<usize>,
}

// Judges everything: the units' tools, then the graph. A fixing run repairs first,
// so the graph is read only after the repairs have settled.
pub fn run(asked: &str, fixing: bool) -> Option<Run> {
    let (outcomes, notes, found) = if fixing {
        let root = PathBuf::from(asked).canonicalize().ok()?;
        let packs = packs::all();
        let repo = units::scan(&root, &packs);
        let (outcomes, notes) = outcomes::judged(&root, &repo, &packs, true);
        (outcomes, notes, graph::graph(asked)?)
    } else {
        let found = graph::graph(asked)?;
        let (outcomes, notes) = outcomes::judged(&found.root, &found.repo, &found.packs, false);
        (outcomes, notes, found)
    };
    let mut all = findings::winnow(&found.defs, &found.edges, &found.repo);
    all.extend(findings::integrity(&found));
    all.extend(ledger::blanket_findings(&found));
    all.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    let counted = ledger::counted(&found);
    let added =
        ledger::reference(&found.root).map(|reference| ledger::added(&counted, &reference).len());
    let counted = counted.len();
    let packages = ledger::packages(&found);
    let third_added = ledger::recorded_packages(&found.root).map(|recorded| {
        packages
            .iter()
            .filter(|name| !recorded.contains(name))
            .count()
    });
    let third = packages.len();
    Some(Run {
        root: found.root,
        units: found.repo.units.len(),
        files: found.repo.files.len(),
        outcomes,
        notes,
        findings: all,
        counted,
        added,
        third,
        third_added,
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

#[cfg(test)]
mod tests {
    use super::tallied;
    use crate::judge::outcomes::Outcome;

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
