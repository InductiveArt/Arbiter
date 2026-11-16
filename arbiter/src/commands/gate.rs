use std::path::Path;
use std::process::ExitCode;

use crate::gate;

pub fn run(arguments: &[String]) -> ExitCode {
    if let Some(command @ "gate") = arguments.first().map(String::as_str) {
        let cold = arguments.iter().any(|argument| argument == "--cold");
        let fixing = arguments.iter().any(|argument| argument == "--fix");
        let asked = arguments
            .iter()
            .skip(1)
            .find(|argument| !argument.starts_with("--"))
            .cloned()
            .unwrap_or_else(|| ".".to_string());
        let gaps = crate::tasks::preflight::preflight(Path::new(&asked));
        if !gaps.is_empty() {
            for gap in &gaps {
                eprintln!("{command}: {gap}");
            }
            eprintln!(
                "{command}: a machine gap is no finding; run the bootstrap, then run the gate again"
            );
            return ExitCode::from(2);
        }
        let Some(run) = gate::run(&asked, fixing, cold) else {
            eprintln!("{command}: no folder at {asked}");
            return ExitCode::from(2);
        };
        println!("{command}: {} units, {} files{}", run.units, run.files, {
            let foreign = &run.graph.repo.foreign;
            if foreign.is_empty() {
                String::new()
            } else {
                let named: Vec<String> = foreign
                    .iter()
                    .map(|place| place.to_string_lossy().to_string())
                    .collect();
                format!(" (foreign: {})", named.join(", "))
            }
        });
        for (check, (passed, failed, warm, seconds)) in gate::tallied(&run.outcomes) {
            let heat = if warm > 0 {
                format!(" ({warm} warm)")
            } else {
                String::new()
            };
            if failed == 0 {
                println!("  {check:15} ok     {passed} units{heat}   {seconds:.1}s");
            } else {
                println!(
                    "  {check:15} failed {failed} of {}{heat}   {seconds:.1}s",
                    passed + failed
                );
            }
        }
        let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
        for finding in &run.findings {
            *counts.entry(finding.check.as_str()).or_insert(0) += 1;
        }
        for (check, count) in &counts {
            println!("  {check:15} {count} findings");
        }
        match run.added {
            None => println!(
                "  {:15} {} counted, no reference recorded",
                "suppressions", run.counted
            ),
            Some(added) => println!(
                "  {:15} {} counted, {added} added since the signature",
                "suppressions", run.counted
            ),
        }
        match run.third_added {
            None => println!(
                "  {:15} {} locked, no reference recorded",
                "third parties", run.third
            ),
            Some(added) => println!(
                "  {:15} {} locked, {added} added since the signature",
                "third parties", run.third
            ),
        }
        for note in &run.notes {
            println!("{command}: {note}");
        }
        if run.outcomes.is_empty() && run.files == 0 {
            println!("{command}: nothing to check here");
            return ExitCode::from(2);
        }
        let place = gate::written(&run);
        let _ = crate::court::write(&run);
        let failed = run
            .outcomes
            .iter()
            .filter(|outcome| !outcome.passed)
            .count();
        if failed == 0 && run.findings.is_empty() {
            println!(
                "{command}: clean — {} checks, no findings",
                run.outcomes.len()
            );
            return ExitCode::SUCCESS;
        }
        let shown = place
            .and_then(|path| path.strip_prefix(&run.root).ok().map(Path::to_path_buf))
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| "the report could not be written".to_string());
        println!(
            "{command}: {failed} checks failed, {} findings — {shown}",
            run.findings.len()
        );
        return ExitCode::FAILURE;
    }
    // Dispatched only for this command, so the guard always holds.
    ExitCode::from(2)
}
