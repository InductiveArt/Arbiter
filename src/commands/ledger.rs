use std::process::ExitCode;

use crate::core::claims;
use crate::core::pack::Suppression;
use crate::ledger;
use crate::packs::graphed;
use crate::records::brief;

pub fn run(arguments: &[String]) -> ExitCode {
    if arguments.first().map(String::as_str) == Some("ledger") {
        if arguments.get(1).map(String::as_str) == Some("sign") {
            let asked = arguments.get(2).cloned().unwrap_or_else(|| ".".to_string());
            let Some(found) = graphed(&asked) else {
                eprintln!("sign: no folder at {asked}");
                return ExitCode::from(2);
            };
            let blankets = ledger::blankets(&found);
            if !blankets.is_empty() {
                eprintln!(
                    "sign: {} blanket suppressions are findings, never records",
                    blankets.len()
                );
                for suppression in &blankets {
                    eprintln!("  {}", shown(suppression));
                }
                return ExitCode::FAILURE;
            }
            let counted = ledger::counted(&found);
            let packages = ledger::packages(&found);
            let spoken = claims::identities(&found);
            let demanded: Vec<String> = brief::read(&found)
                .map(|reading| {
                    reading
                        .needs
                        .iter()
                        .map(|need| format!("{} | {}", need.case, need.need))
                        .collect()
                })
                .unwrap_or_default();
            match ledger::record(&found.root, &counted, &packages, &spoken, &demanded) {
                Ok(path) => {
                    println!(
                        "sign: {} suppressions, {} third parties, {} claims and {} needs recorded in {}",
                        counted.len(),
                        packages.len(),
                        spoken.len(),
                        demanded.len(),
                        path.to_string_lossy()
                    );
                    for pack in &found.packs {
                        for line in pack.record(&found.root, &found.repo) {
                            println!("sign: {line}");
                        }
                    }
                    return ExitCode::SUCCESS;
                }
                Err(error) => {
                    eprintln!("sign: the reference could not be written: {error}");
                    return ExitCode::from(2);
                }
            }
        }
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graphed(&asked) else {
            eprintln!("ledger: no folder at {asked}");
            return ExitCode::from(2);
        };
        return ledgered(&found);
    }
    // Dispatched only for this command, so the guard always holds.
    ExitCode::from(2)
}

// The signature's account: everything signing records, counted against what
// stands today, with each delta listed where one exists.
fn ledgered(found: &crate::core::graph::Graph) -> ExitCode {
    let counted = ledger::counted(found);
    let reference = ledger::reference(&found.root);
    match &reference {
        None => {
            println!("{:14} {} counted", "suppressions", counted.len());
            for suppression in &counted {
                println!("  {}", shown(suppression));
            }
        }
        Some(reference) => {
            let extra = ledger::added(&counted, reference);
            println!(
                "{:14} {} counted, {} added since the signature",
                "suppressions",
                counted.len(),
                extra.len()
            );
            for suppression in &extra {
                println!("  {}", shown(suppression));
            }
        }
    }
    let packages = ledger::packages(found);
    match ledger::recorded_packages(&found.root) {
        None => println!("{:14} {} locked", "third parties", packages.len()),
        Some(recorded) => {
            let fresh: Vec<&String> = packages
                .iter()
                .filter(|name| !recorded.contains(*name))
                .collect();
            println!(
                "{:14} {} locked, {} added since the signature",
                "third parties",
                packages.len(),
                fresh.len()
            );
            for name in fresh {
                println!("  {name}");
            }
        }
    }
    let spoken = claims::identities(found).len();
    match claims::recorded(&found.root) {
        None => println!("{:14} {} spoken", "claims", spoken),
        Some(_) => {
            let lost = claims::gone(found);
            println!(
                "{:14} {} spoken, {} vanished since the signature",
                "claims",
                spoken,
                lost.len()
            );
            for claim in &lost {
                println!("  '{claim}'");
            }
        }
    }
    let blankets = ledger::blankets(found);
    if !blankets.is_empty() {
        println!(
            "{:14} {} refused, never recorded",
            "blankets",
            blankets.len()
        );
        for suppression in &blankets {
            println!("  {}", shown(suppression));
        }
    }
    if reference.is_none() {
        println!("no reference recorded; arbiter ledger sign records today's");
    }
    ExitCode::SUCCESS
}

fn shown(suppression: &Suppression) -> String {
    format!(
        "{}:{} {}",
        suppression.file.to_string_lossy(),
        suppression.line,
        suppression.text
    )
}
