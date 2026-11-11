use std::path::Path;
use std::process::ExitCode;

use crate::core::claims;
use crate::packs::graphed;
use crate::records::brief;

pub fn run(arguments: &[String]) -> ExitCode {
    if arguments.first().map(String::as_str) == Some("brief") {
        let (case, asked) = match (arguments.get(1), arguments.get(2)) {
            (Some(first), second) if !Path::new(first).is_dir() => (
                Some(first.clone()),
                second.cloned().unwrap_or_else(|| ".".to_string()),
            ),
            (first, _) => (None, first.cloned().unwrap_or_else(|| ".".to_string())),
        };
        let Some(found) = graphed(&asked) else {
            eprintln!("brief: no folder at {asked}");
            return ExitCode::from(2);
        };
        let Some(reading) = brief::read(&found) else {
            return unwritten(&found);
        };
        if let Some(case) = case {
            return slice(&case, &reading);
        }
        println!(
            "brief: {} cases — {} tested, {} planned, {} manual, {} unanswered; \
             {} claims, {} unwired",
            reading.cases.len(),
            reading
                .needs
                .iter()
                .filter(|need| need.state == brief::State::Tested)
                .count(),
            reading
                .needs
                .iter()
                .filter(|need| need.state == brief::State::Planned)
                .count(),
            reading
                .needs
                .iter()
                .filter(|need| need.state == brief::State::Manual)
                .count(),
            reading
                .needs
                .iter()
                .filter(|need| need.state == brief::State::Unanswered)
                .count(),
            reading.claims,
            reading.unwired.len(),
        );
        let mut case = String::new();
        for need in &reading.needs {
            if need.case != case {
                println!();
                println!("{}", need.case);
                case = need.case.clone();
            }
            let state = match need.state {
                brief::State::Tested => "tested",
                brief::State::Planned => "planned",
                brief::State::Manual => "manual",
                brief::State::Unanswered => "unanswered",
            };
            println!("  {state:10} {}", need.need);
        }
        if !reading.unwired.is_empty() {
            println!();
            println!("unwired claims, each with the binding to paste:");
            for (file, line, identity) in &reading.unwired {
                println!("  {}:{line} \"{identity}\"", file.to_string_lossy());
            }
        }
        return ExitCode::SUCCESS;
    }
    // Dispatched only for this command, so the guard always holds.
    ExitCode::from(2)
}

// With no brief written, the whole tree is the unwired world: the claims an
// agent reads to draft the first cases.
fn unwritten(found: &crate::core::graph::Graph) -> ExitCode {
    let (claimed, silent) = claims::heard(found);
    let behaviors: usize = claimed.values().map(Vec::len).sum();
    println!("brief: no brief written; {behaviors} claims unwired");
    for (unit, list) in &claimed {
        let shown = if unit.as_os_str().is_empty() {
            ".".to_string()
        } else {
            unit.to_string_lossy().to_string()
        };
        println!();
        println!("{shown} - {}", list.len());
        let mut last = None;
        let mut told: Vec<String> = Vec::new();
        for one in list {
            if last != Some(&one.file) {
                println!("  {}", one.file.to_string_lossy());
                last = Some(&one.file);
                told.clear();
            }
            for (depth, frame) in one.claim.path.iter().enumerate() {
                if told.len() > depth && told[depth] == *frame {
                    continue;
                }
                told.truncate(depth);
                told.push(frame.clone());
                println!("{}{frame}", "  ".repeat(depth + 2));
            }
            told.truncate(one.claim.path.len());
            println!(
                "{}{}",
                "  ".repeat(one.claim.path.len() + 2),
                one.claim.sentence
            );
        }
    }
    if !silent.is_empty() {
        let names: Vec<String> = silent
            .iter()
            .map(|unit| {
                if unit.as_os_str().is_empty() {
                    ".".to_string()
                } else {
                    unit.to_string_lossy().to_string()
                }
            })
            .collect();
        println!();
        println!("claims nothing: {}", names.join(", "));
    }
    ExitCode::SUCCESS
}

// One case whole: its needs, their states, and what each binds.
fn slice(case: &str, reading: &crate::records::brief::Reading) -> ExitCode {
    let needs: Vec<&crate::records::brief::Need> = reading
        .needs
        .iter()
        .filter(|need| need.case == case)
        .collect();
    if needs.is_empty() {
        eprintln!("brief: no case named {case}");
        return ExitCode::from(2);
    }
    println!("{case} — {} needs", needs.len());
    for need in needs {
        let state = match need.state {
            brief::State::Tested => "tested",
            brief::State::Planned => "planned",
            brief::State::Manual => "manual",
            brief::State::Unanswered => "unanswered",
        };
        println!();
        println!("  {state:10} {}", need.need);
        for entry in &need.entries {
            println!("             {entry}");
        }
    }
    ExitCode::SUCCESS
}
