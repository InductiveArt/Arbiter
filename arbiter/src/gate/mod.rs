use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::core::graph;
use crate::core::tools::Outcome;
use crate::core::units;
use crate::domain::finding::Finding;
use crate::ledger;
use crate::packs;

// One whole gate run over a repository, with the ledger's footer.
pub struct Run {
    pub root: PathBuf,
    pub graph: crate::core::graph::Graph,
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
    // The run's task tree: every task that ran, with its cost and subtree.
    pub steps: Vec<crate::core::task::Node>,
}

// Runs everything: the units' tools, then the graph's laws. A fixing run repairs first,
// so the graph is read only after the repairs have settled, the tools' included.
pub fn run(asked: &str, fixing: bool, cold: bool) -> Option<Run> {
    let (outcomes, notes, dialed, found) = if fixing {
        let root = PathBuf::from(asked).canonicalize().ok()?;
        let packs = packs::all();
        let repo = units::scan(&root, &packs);
        // Packs repair their own findings before the tools format the result;
        // the graph and the warm keys read the repaired state.
        let mut mended = Vec::new();
        for pack in &packs {
            mended.extend(pack.mend(&root, &repo));
        }
        let found = graph::graph(asked, packs)?;
        let warmth = heated(&found, cold);
        let (outcomes, mut notes, dialed) = crate::tasks::tools::checked(
            &found.root,
            &found.repo,
            &found.packs,
            true,
            cold,
            warmth.as_ref(),
        );
        notes.extend(mended);
        // A tool's repair rewrites files the graph was read from.
        let found = crate::packs::graphed(asked)?;
        (outcomes, notes, dialed, found)
    } else {
        let found = crate::packs::graphed(asked)?;
        let warmth = heated(&found, cold);
        let (outcomes, notes, dialed) = crate::tasks::tools::checked(
            &found.root,
            &found.repo,
            &found.packs,
            false,
            cold,
            warmth.as_ref(),
        );
        (outcomes, notes, dialed, found)
    };
    let mut journal = crate::core::task::Journal::new();
    journal.noted(
        "scan",
        found.scan_seconds,
        vec![format!(
            "{} units, {} files",
            found.repo.units.len(),
            found.repo.files.len()
        )],
    );
    journal.noted("resolve", found.resolve_seconds, Vec::new());
    journal.run("tools", |tools| {
        for outcome in outcomes.iter().filter(|outcome| !outcome.warm) {
            tools.noted(
                &format!("{}/{}", outcome.unit, outcome.check),
                outcome.seconds,
                Vec::new(),
            );
        }
        crate::core::task::Done::ok(())
    });
    let mut all = journal.run("laws", |j| {
        let mut all = dialed;
        for law in crate::tasks::laws::all() {
            all.extend(j.run(law.task, |_| crate::core::task::Done::ok((law.run)(&found))));
        }
        for pack in &found.packs {
            all.extend(j.run(pack.label(), |_| {
                crate::core::task::Done::ok(pack.integrity(&found.root, &found.repo))
            }));
        }
        crate::core::task::Done::ok(all)
    });
    all.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    let (counted, added, third, third_added) = journal.run("ledger", |_| {
        let counted = ledger::counted(&found);
        let added = ledger::reference(&found.root)
            .map(|reference| ledger::added(&counted, &reference).len());
        let packages = ledger::packages(&found);
        let third_added = ledger::recorded_packages(&found.root).map(|recorded| {
            packages
                .iter()
                .filter(|name| !recorded.contains(name))
                .count()
        });
        crate::core::task::Done::ok((counted.len(), added, packages.len(), third_added))
    });
    let units = found.repo.units.len();
    let files = found.repo.files.len();
    Some(Run {
        root: found.root.clone(),
        graph: found,
        units,
        files,
        outcomes,
        notes,
        findings: all,
        counted,
        added,
        third,
        third_added,
        steps: journal.tree(),
    })
}

// Per check: how many units passed, how many failed, and the seconds they took.
pub fn tallied(outcomes: &[Outcome]) -> BTreeMap<String, (usize, usize, usize, f64)> {
    let mut tallies: BTreeMap<String, (usize, usize, usize, f64)> = BTreeMap::new();
    for outcome in outcomes {
        let entry = tallies
            .entry(outcome.check.clone())
            .or_insert((0, 0, 0, 0.0));
        if outcome.passed {
            entry.0 += 1;
        } else {
            entry.1 += 1;
        }
        if outcome.warm {
            entry.2 += 1;
        } else {
            entry.3 += outcome.seconds;
        }
    }
    tallies
}

// The warm store, built from the resolved graph. A cold run reads nothing,
// yet still records: a full cold run is the best evidence a store can hold.
fn heated(found: &graph::Graph, cold: bool) -> Option<crate::core::warm::Warmth> {
    let defs_units: Vec<usize> = found.defs.iter().map(|def| def.unit).collect();
    let mut warmth = crate::core::warm::warmth(
        &found.root,
        &found.repo,
        &found.packs,
        &found.edges,
        &defs_units,
    );
    if cold {
        warmth.chill();
    }
    Some(warmth)
}

// Writes the run's detail where the dashboard points: a report of sections to read
// one at a time, and the same material as JSON lines. The folder never commits.
pub fn written(run: &Run) -> Option<PathBuf> {
    seeded(&run.root);
    let folder = run.root.join(".arbiter").join("last");
    std::fs::create_dir_all(&folder).ok()?;
    std::fs::write(folder.join(".gitignore"), "*\n").ok()?;
    let mut report = String::from("# arbiter\n");
    for outcome in &run.outcomes {
        if outcome.passed {
            continue;
        }
        report.push_str(&format!("\n## {} — {}\n\n", outcome.check, outcome.unit));
        for line in outcome.output.lines() {
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
                finding.line.unwrap_or(0),
                finding.rule.as_deref().unwrap_or(""),
                finding.reason
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
                "line": finding.line.unwrap_or(0),
                "rule": finding.rule,
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
            entry["output"] = serde_json::Value::String(outcome.output.clone());
            entry["warm"] = serde_json::Value::Bool(outcome.warm);
        }
        lines.push_str(&entry.to_string());
        lines.push('\n');
    }
    std::fs::write(folder.join("findings.jsonl"), lines).ok()?;
    Some(folder.join("report.md"))
}

const UNITS_SEED: &str = "\
# The unit graph, declared. The gate holds the resolved imports to this file\n\
# one unit at a time: a unit absent here is unchecked, so adopt at your pace.\n\
#\n\
# [units.\"libs/example\"]\n\
# # Who may import this unit: exact paths, a folder and below as \"apps/*\", or \"*\".\n\
# visibility = [\"apps/*\"]\n\
# # The units this one uses, held both ways: using an undeclared unit is a\n\
# # finding, and so is declaring one never used.\n\
# deps = [\"libs/other\"]\n\
#\n\
# [units.\"libs/example\".inner.\"src/api\"]\n\
# # Inside the unit the same two consents apply to a folder or a file, and a\n\
# # child only tightens what the unit allows. `//` reaches from the repo root.\n\
# visibility = [\"src/app/*\"]\n\
# deps = [\"src/domain\", \"//libs/other\"]\n";

const SUPPRESSIONS_SEED: &str = "\
# Record suppressions. Each entry quiets one rule for the packages it names,\n\
# in the units it states, and is counted in the ledger like any suppression.\n\
# An entry that quiets nothing is stale; a form wider than its named subjects\n\
# is a blanket, refused and never recorded.\n\
#\n\
# [[suppression]]\n\
# rule = \"unused-dependency\"\n\
# language = \"python\"\n\
# packages = [\"coverage\"]\n\
# units = [\"*\"]\n\
#\n\
# [[suppression]]\n\
# rule = \"sibling-cycle\"\n\
# unit = \".\"\n\
# # The whole member set, exactly: a third member joining the cycle fires fresh.\n\
# members = [\"src/a\", \"src/b\"]\n";

const LICENSES_SEED: &str = "\
# The licenses third parties may carry. Until an allow list is written here,\n\
# no license policy exists; once written, every dependency whose license the\n\
# list does not name is a finding. An empty list allows nothing.\n\
#\n\
# allow = [\n\
#   \"MIT\",\n\
#   \"Apache-2.0\",\n\
# ]\n";

// The decision files, seeded as comments the first time the gate runs: an
// invitation that declares nothing, never overwritten once present.
pub fn seeded(root: &std::path::Path) {
    let folder = root.join(".arbiter");
    if std::fs::create_dir_all(&folder).is_err() {
        return;
    }
    for (name, seed) in [
        ("units.toml", UNITS_SEED),
        ("licenses.toml", LICENSES_SEED),
        ("suppressions.toml", SUPPRESSIONS_SEED),
    ] {
        let path = folder.join(name);
        if !path.exists() {
            let _ = std::fs::write(path, seed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tallied;
    use crate::core::tools::Outcome;

    fn outcome(check: &str, passed: bool, seconds: f64) -> Outcome {
        Outcome {
            unit: "app".to_string(),
            pack: "rust",
            check: check.to_string(),
            passed,
            seconds,
            output: String::new(),
            warm: false,
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
        assert_eq!(tallies["lint"], (1, 1, 0, 3.0));
        assert_eq!(tallies["test"], (1, 0, 0, 3.0));
    }
}
