use std::path::Path;
use std::process::ExitCode;

use crate::domain::finding::Finding;
use crate::packs::graphed;

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

// No feature owns the first word: it is a rule to filter by, a folder to read,
// or nothing, in which case the registry lists what find can answer.
fn bare(rest: &[String]) -> ExitCode {
    let folder = |at: usize| rest.get(at).cloned().unwrap_or_else(|| ".".to_string());
    let (rule, asked) = match rest.first().map(String::as_str) {
        None => (None, ".".to_string()),
        Some(word) if crate::tasks::laws::findable(word) => (Some(word.to_string()), folder(1)),
        Some(word) if crate::tasks::laws::drillable(word) => {
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
    let all = crate::tasks::laws::findings(&found);
    emit(&all, rule.as_deref());
    ExitCode::SUCCESS
}

fn emit(findings: &[Finding], rule: Option<&str>) {
    for finding in findings {
        if rule.is_some_and(|rule| finding.rule.as_deref() != Some(rule)) {
            continue;
        }
        println!("{}", finding.written());
    }
}
