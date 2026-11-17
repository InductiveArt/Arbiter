use std::collections::BTreeMap;
use std::path::Path;

use crate::domain::check_outcome::CheckOutcome;
use crate::report::gate_run::GateRun;

// What the gate tells before its verdict: the units and files read and the
// folders left to their own gate, each check's tally, each check's findings,
// the ledger's two counts, and every note.
pub fn told(command: &str, run: &GateRun) -> Vec<String> {
    let mut told = Vec::new();
    let foreign = &run.graph.repo.foreign;
    let named = if foreign.is_empty() {
        String::new()
    } else {
        let named: Vec<String> = foreign
            .iter()
            .map(|place| place.to_string_lossy().to_string())
            .collect();
        format!(" (foreign: {})", named.join(", "))
    };
    told.push(format!(
        "{command}: {} units, {} files{named}",
        run.units(),
        run.files()
    ));
    for (check, (passed, failed, warm, seconds)) in tallied(&run.outcomes) {
        let heat = if warm > 0 {
            format!(" ({warm} warm)")
        } else {
            String::new()
        };
        if failed == 0 {
            told.push(format!(
                "  {check:15} ok     {passed} units{heat}   {seconds:.1}s"
            ));
        } else {
            told.push(format!(
                "  {check:15} failed {failed} of {}{heat}   {seconds:.1}s",
                passed + failed
            ));
        }
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for finding in &run.findings {
        *counts.entry(finding.check.as_str()).or_insert(0) += 1;
    }
    for (check, count) in &counts {
        told.push(format!("  {check:15} {count} findings"));
    }
    let ledger = &run.ledger;
    told.push(match ledger.added {
        None => format!(
            "  {:15} {} counted, no reference recorded",
            "suppressions",
            ledger.counted.len()
        ),
        Some(added) => format!(
            "  {:15} {} counted, {added} added since the signature",
            "suppressions",
            ledger.counted.len()
        ),
    });
    told.push(match ledger.third_added {
        None => format!(
            "  {:15} {} locked, no reference recorded",
            "third parties", ledger.third
        ),
        Some(added) => format!(
            "  {:15} {} locked, {added} added since the signature",
            "third parties", ledger.third
        ),
    });
    told.extend(run.notes.iter().map(|note| format!("{command}: {note}")));
    told
}

// The last line: clean, or how many checks failed and how many findings were
// found, and where the report holding them is.
pub fn verdict(command: &str, run: &GateRun, place: Option<&Path>) -> String {
    if run.clean() {
        return format!(
            "{command}: clean — {} checks, no findings",
            run.outcomes.len()
        );
    }
    let shown = place
        .and_then(|path| path.strip_prefix(run.root()).ok())
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "the report could not be written".to_string());
    format!(
        "{command}: {} checks failed, {} findings — {shown}",
        run.failed(),
        run.findings.len()
    )
}

// Per check: how many units passed, how many failed, how many a stored run
// answered for, and the seconds the others took.
pub fn tallied(outcomes: &[CheckOutcome]) -> BTreeMap<String, (usize, usize, usize, f64)> {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::tallied;
    use crate::domain::check_outcome::CheckOutcome;

    fn outcome(check: &str, passed: bool, seconds: f64, warm: bool) -> CheckOutcome {
        CheckOutcome {
            check: check.to_string(),
            unit: PathBuf::from("app"),
            language: "rust".to_string(),
            passed,
            seconds,
            output: String::new(),
            warm,
        }
    }

    #[test]
    fn a_tally_counts_passes_and_failures_per_check_and_the_time_of_what_ran() {
        let outcomes = [
            outcome("lint", true, 1.0, false),
            outcome("lint", false, 2.0, false),
            outcome("test", true, 3.0, false),
            outcome("test", true, 9.0, true),
        ];
        let tallies = tallied(&outcomes);
        assert_eq!(tallies["lint"], (1, 1, 0, 3.0));
        assert_eq!(
            tallies["test"],
            (2, 0, 1, 3.0),
            "a stored run took no time in this one"
        );
    }
}
