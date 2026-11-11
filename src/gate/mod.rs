pub mod tools;

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::core::claims;
use crate::core::graph;
use crate::core::pack::Finding;
use crate::core::units;
use crate::gate::tools::Outcome;
use crate::laws::findings;
use crate::ledger;
use crate::packs;
use crate::records::brief;

// One whole judgment of a repository: both planes, with the ledger's footer.
// What the gate says of the brief: the step skipped, no brief written yet, or
// the counted states of a read one.
pub enum BriefLine {
    Skipped,
    Unwritten,
    Read {
        cases: usize,
        tested: usize,
        planned: usize,
        manual: usize,
        unanswered: usize,
        unwired: usize,
    },
}

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
    pub brief: BriefLine,
    pub claims: usize,
    pub claims_vanished: Option<usize>,
    pub needs_stated: usize,
    // Needs added since the reference; None when none was recorded yet.
    pub needs_added: Option<usize>,
    pub third: usize,
    // Third parties added since the reference; None when none was recorded yet.
    pub third_added: Option<usize>,
    // The run's task tree: every task that ran, with its cost and subtree.
    pub steps: Vec<crate::core::task::Node>,
}

// Judges everything: the units' tools, then the graph. A fixing run repairs first,
// so the graph is read only after the repairs have settled.
pub fn run(asked: &str, fixing: bool, cold: bool, briefless: bool) -> Option<Run> {
    let (outcomes, mut notes, dialed, found) = if fixing {
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
        let (outcomes, mut notes, dialed) = tools::judged(
            &found.root,
            &found.repo,
            &found.packs,
            true,
            cold,
            warmth.as_ref(),
        );
        notes.extend(mended);
        (outcomes, notes, dialed, found)
    } else {
        let found = crate::packs::graphed(asked)?;
        let warmth = heated(&found, cold);
        let (outcomes, notes, dialed) = tools::judged(
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
    let commissioned = brief::commissioned(&found);
    let (mut all, nascent) = journal.run("judge", |_| {
        let (winnowed, nascent) =
            findings::winnow(&found.defs, &found.edges, &found.repo, &commissioned);
        let mut all = winnowed;
        all.extend(dialed);
        all.extend(findings::integrity(&found));
        all.extend(ledger::blanket_findings(&found));
        all.extend(ledger::broken_record(&found.root));
        crate::core::task::Done::ok((all, nascent))
    });
    notes.extend(nascent);
    let mut needs_spoken: Vec<String> = Vec::new();
    let brief = if briefless {
        BriefLine::Skipped
    } else {
        match brief::read(&found) {
            None => BriefLine::Unwritten,
            Some(reading) => {
                needs_spoken = reading
                    .needs
                    .iter()
                    .map(|need| format!("{} | {}", need.case, need.need))
                    .collect();
                let count = |state: brief::State| {
                    reading
                        .needs
                        .iter()
                        .filter(|need| need.state == state)
                        .count()
                };
                BriefLine::Read {
                    cases: reading.cases.len(),
                    tested: count(brief::State::Tested),
                    planned: count(brief::State::Planned),
                    manual: count(brief::State::Manual),
                    unanswered: count(brief::State::Unanswered),
                    unwired: reading.unwired.len(),
                }
            }
        }
    };
    if !briefless {
        all.extend(brief::findings(&found));
    }
    let gone = journal.run("claims", |_| {
        crate::core::task::Done::ok(claims::vanished(&found))
    });
    let claims = claims::identities(&found).len();
    let claims_vanished = claims::recorded(&found.root).map(|_| gone.len());
    let needs_stated = needs_spoken.len();
    let needs_added = ledger::recorded_needs(&found.root).map(|recorded| {
        needs_spoken
            .iter()
            .filter(|need| !recorded.contains(need))
            .count()
    });
    all.extend(gone);
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
        brief,
        claims,
        claims_vanished,
        needs_stated,
        needs_added,
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
                "rule": finding.name,
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

const PLANS_SEED: &str = "\
# The plans: deployment declared before it is possible. Services, the state\n\
# they keep, the machines expected, and the plans placing them - judged by\n\
# the gate, consumed by any actuator or none. needs order bring-up and must\n\
# form a DAG; channels are steady-state calls, consented by accepts.\n\
#\n\
# [[service]]\n\
# name = \"api\"\n\
# unit = \"apps/api\"\n\
# needs = [\"postgres\"]\n\
# channels = [{ to = \"worker\", contract = \"jobs.v1.JobService\" }]\n\
# accepts = [\"web\"]\n\
# volumes = [\"api-data\"]\n\
# cpu = 0.5\n\
# memory_mb = 512\n\
#\n\
# [[volume]]\n\
# name = \"api-data\"\n\
# backup = \"daily\"\n\
#\n\
# [[machine]]\n\
# name = \"cell-1\"\n\
# cpu = 8\n\
# memory_mb = 16384\n\
#\n\
# [[plan]]\n\
# name = \"local\"\n\
# place = [{ service = \"api\", on = \"cell-1\" }]\n";

const BRIEF_SEED: &str = "\
# The brief: the needs this software answers, as cases of activity, each need\n\
# bound to the claims proving it. A binding is a claim's identity from\n\
# arbiter brief ., or a frame prefix covering every claim under it. With one\n\
# case written the gate holds every need answered and every claim wired.\n\
#\n\
# [[case]]\n\
# name = \"joining\"\n\
# story = \"a peer joins a room and collaborates from the present state\"\n\
#\n\
#   [[case.need]]\n\
#   need = \"a late joiner stands in the present before its first render\"\n\
#   claims = [\"libs/relay | wire | a late joiner stands in the present\"]\n\
#\n\
#   [[case.need]]\n\
#   need = \"a full room refuses the next peer\"\n\
#   planned = true\n\
";

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
        ("plans.toml", PLANS_SEED),
        ("brief.toml", BRIEF_SEED),
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
    use crate::gate::tools::Outcome;

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
