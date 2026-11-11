use std::path::Path;
use std::process::ExitCode;

use crate::core::claims;
use crate::core::pack;
use crate::core::pack::Finding;
use crate::laws::findings::integrity;
use crate::laws::findings::winnow;
use crate::ledger;
use crate::packs::graphed;
use crate::records::brief;

// The one query door: a task name names a report, a rule names one law's
// instances, bare is every tool-free finding. find routes to the task that owns
// the name and never reads what the task reads; a find reads, never writes.
pub fn run(rest: &[String]) -> ExitCode {
    let subcommand = rest.first().map(String::as_str);
    for task in crate::tasks::all() {
        if Some(task.name) == subcommand {
            return (task.run)(&rest[1..]);
        }
    }
    bare(rest)
}

// No feature owns the first word: it is a rule to filter by, a folder to judge,
// or nothing, in which case the registry lists what find can answer.
fn bare(rest: &[String]) -> ExitCode {
    let folder = |at: usize| rest.get(at).cloned().unwrap_or_else(|| ".".to_string());
    let (rule, asked) = match rest.first().map(String::as_str) {
        None => (None, ".".to_string()),
        Some(word) if pack::findable(word) => (Some(word.to_string()), folder(1)),
        Some(word) if pack::drillable(word) => {
            eprintln!(
                "find: {word} fires through the gate's outcomes; \
                 run arbiter gate . and read .arbiter/last/report.md"
            );
            return ExitCode::from(2);
        }
        Some(word) if Path::new(word).is_dir() => (None, word.to_string()),
        Some(word) => {
            eprintln!("find: nothing named {word}; find takes a rule from arbiter rules, or:");
            for task in crate::tasks::all() {
                eprintln!("  {:8} {}", task.name, task.line);
            }
            return ExitCode::from(2);
        }
    };
    let Some(found) = graphed(&asked) else {
        eprintln!("find: no folder at {asked}");
        return ExitCode::from(2);
    };
    let commissioned = brief::commissioned(&found);
    let (winnowed, _) = winnow(&found.defs, &found.edges, &found.repo, &commissioned);
    let mut all = winnowed;
    all.extend(integrity(&found));
    all.extend(ledger::blanket_findings(&found));
    all.extend(ledger::broken_record(&found.root));
    all.extend(claims::vanished(&found));
    all.extend(brief::findings(&found));
    all.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    emit(&all, rule.as_deref());
    ExitCode::SUCCESS
}

fn emit(findings: &[Finding], rule: Option<&str>) {
    for finding in findings {
        if rule.is_some_and(|rule| finding.name != rule) {
            continue;
        }
        let shown = finding.file.to_string_lossy();
        println!(
            "{}",
            serde_json::json!({
                "check": finding.check,
                "file": shown,
                "line": finding.line,
                "rule": finding.name,
                "reason": finding.reason,
            })
        );
    }
}
