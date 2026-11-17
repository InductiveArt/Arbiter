use std::path::Path;
use std::process::ExitCode;

use task_runner::execution::runner::Runner;

use crate::core::seeds;
use crate::gate;
use crate::report::dashboard;
use crate::report::files;

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
        // This binary performs every attempt of the gate's plans.
        let runner = Runner::new().task_arguments(&["attempt"]);
        let run = match gate::run::run(&asked, fixing, cold, runner) {
            Ok(run) => run,
            Err(reason) => {
                eprintln!("{command}: {reason}");
                return ExitCode::from(2);
            }
        };
        for line in dashboard::told(command, &run) {
            println!("{line}");
        }
        if run.gated_nothing() {
            println!("{command}: nothing to check here");
            return ExitCode::from(2);
        }
        seeds::seeded(run.root());
        let place = files::written(&run);
        let _ = crate::court::write(&run);
        println!("{}", dashboard::verdict(command, &run, place.as_deref()));
        return if run.clean() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    // Dispatched only for this command, so the guard always holds.
    ExitCode::from(2)
}
