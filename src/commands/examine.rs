use std::process::ExitCode;

use crate::examine;
use crate::packs::graphed;

pub fn run(arguments: &[String]) -> ExitCode {
    if arguments.first().map(String::as_str) == Some("examine") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graphed(&asked) else {
            eprintln!("examine: no folder at {asked}");
            return ExitCode::from(2);
        };
        match examine::run(&found) {
            Ok(testimonies) => {
                let mut refuted = false;
                for testimony in &testimonies {
                    println!("case {} — sequence {}", testimony.case, examine::SEQUENCE);
                    for step in &testimony.steps {
                        let mark = if step.pass { "✓" } else { "✗" };
                        println!("  {mark} {} — {}", step.act, step.saw);
                    }
                    let verdict = if testimony.passed {
                        "verified"
                    } else {
                        "refuted"
                    };
                    for need in &testimony.needs {
                        println!("  {verdict}: {need}");
                    }
                    refuted |= !testimony.passed;
                }
                println!(
                    "examine: receipts — {}",
                    found
                        .root
                        .join(".arbiter")
                        .join("last")
                        .join("receipts.json")
                        .display()
                );
                return ExitCode::from(u8::from(refuted));
            }
            Err(reason) => {
                eprintln!("examine: {reason}");
                return ExitCode::from(2);
            }
        }
    }
    // Dispatched only for this command, so the guard always holds.
    ExitCode::from(2)
}
