use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::domain::check_outcome::CheckOutcome;
use crate::domain::finding::Finding;
use crate::domain::unit::shown;
use crate::report::gate_run::GateRun;

// The run's detail where the dashboard points: a report of sections to read
// one at a time, and the same material as JSON lines. The folder never
// commits.
pub fn written(run: &GateRun) -> Option<PathBuf> {
    let folder = run.root().join(".arbiter").join("last");
    fs::create_dir_all(&folder).ok()?;
    fs::write(folder.join(".gitignore"), "*\n").ok()?;
    fs::write(
        folder.join("report.md"),
        report(&run.outcomes, &run.findings),
    )
    .ok()?;
    fs::write(
        folder.join("findings.jsonl"),
        lines(&run.outcomes, &run.findings),
    )
    .ok()?;
    Some(folder.join("report.md"))
}

// One section for each check that failed, holding all it printed, then one
// for each file holding a finding, a line per finding.
pub fn report(outcomes: &[CheckOutcome], findings: &[Finding]) -> String {
    let mut report = String::from("# arbiter\n");
    for outcome in outcomes.iter().filter(|outcome| !outcome.passed) {
        report.push_str(&format!(
            "\n## {} — {}\n\n",
            outcome.check,
            shown(&outcome.unit)
        ));
        for line in outcome.output.lines() {
            report.push_str(line);
            report.push('\n');
        }
    }
    let mut by_file: BTreeMap<&PathBuf, Vec<&Finding>> = BTreeMap::new();
    for finding in findings {
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
    report
}

// Each finding as a JSON line, then each check's outcome; a failed outcome
// carries all it printed and whether a stored run answered for it.
pub fn lines(outcomes: &[CheckOutcome], findings: &[Finding]) -> String {
    let mut lines = String::new();
    for finding in findings {
        let mut entry = finding.written();
        entry["kind"] = serde_json::Value::from("finding");
        lines.push_str(&entry.to_string());
        lines.push('\n');
    }
    for outcome in outcomes {
        let mut entry = serde_json::json!({
            "kind": "outcome",
            "check": outcome.check,
            "unit": shown(&outcome.unit),
            "pack": outcome.language,
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
    lines
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::lines;
    use super::report;
    use crate::domain::check_outcome::CheckOutcome;
    use crate::domain::finding::Finding;

    fn outcome(check: &str, unit: &str, passed: bool, output: &str) -> CheckOutcome {
        CheckOutcome {
            check: check.to_string(),
            unit: PathBuf::from(unit),
            language: "rust".to_string(),
            passed,
            seconds: 1.5,
            output: output.to_string(),
            warm: false,
        }
    }

    fn finding(file: &str, line: Option<u32>, rule: Option<&str>) -> Finding {
        Finding {
            check: "economy".to_string(),
            rule: rule.map(str::to_string),
            file: PathBuf::from(file),
            line,
            reason: "told".to_string(),
        }
    }

    #[test]
    fn the_report_holds_a_section_per_failed_check_then_one_per_file_with_findings() {
        let outcomes = [
            outcome("lint_rust", "", false, "error: one\nerror: two"),
            outcome("test_rust", "libs/a", true, ""),
        ];
        let findings = [
            finding("a.rs", Some(3), Some("dead-symbol")),
            finding("a.rs", None, None),
            finding("b.rs", Some(1), Some("cycle")),
        ];
        assert_eq!(
            report(&outcomes, &findings),
            "# arbiter\n\n## lint_rust — .\n\nerror: one\nerror: two\n\n## a.rs\n\n\
             3 dead-symbol told\n0  told\n\n## b.rs\n\n1 cycle told\n"
        );
        assert_eq!(
            report(&[], &[]),
            "# arbiter\n",
            "a clean run leaves only the title"
        );
    }

    #[test]
    fn each_finding_then_each_outcome_is_one_json_line_and_only_a_failed_one_carries_its_output() {
        let outcomes = [
            outcome("lint_rust", "", false, "error"),
            outcome("test_rust", "libs/a", true, ""),
        ];
        assert_eq!(
            lines(&outcomes, &[finding("a.rs", None, Some("dead-symbol"))]),
            "{\"check\":\"economy\",\"file\":\"a.rs\",\"kind\":\"finding\",\"line\":0,\
             \"reason\":\"told\",\"rule\":\"dead-symbol\"}\n\
             {\"check\":\"lint_rust\",\"kind\":\"outcome\",\"output\":\"error\",\"pack\":\"rust\",\
             \"passed\":false,\"seconds\":1.5,\"unit\":\".\",\"warm\":false}\n\
             {\"check\":\"test_rust\",\"kind\":\"outcome\",\"pack\":\"rust\",\"passed\":true,\
             \"seconds\":1.5,\"unit\":\"libs/a\"}\n"
        );
    }
}
