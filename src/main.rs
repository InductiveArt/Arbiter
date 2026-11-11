use std::env;
use std::process::ExitCode;

use arbiter::commands;

fn main() -> ExitCode {
    sigpipe();
    let arguments: Vec<String> = env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        Some("fix") => {
            eprintln!("fix became a mode: arbiter gate --fix");
            ExitCode::from(2)
        }
        Some("gate") => commands::gate::run(&arguments),
        Some("find") => commands::find::run(&arguments[1..]),
        Some("brief") => commands::brief::run(&arguments),
        Some("examine") => commands::examine::run(&arguments),
        Some("adopt") => commands::adopt::run(&arguments),
        Some("rules") => commands::rules::run(&arguments),
        Some("ledger") => commands::ledger::run(&arguments),
        Some(command) => {
            eprintln!("arbiter: no command named {command}");
            commands::table::taught();
            ExitCode::from(2)
        }
        None => {
            commands::table::taught();
            ExitCode::from(2)
        }
    }
}

// A closed pipe ends the read, not the program: piping a report into `head` is
// a normal way to read it, never a panic.
fn sigpipe() {
    #[cfg(unix)]
    unsafe {
        unsafe extern "C" {
            fn signal(number: i32, handler: usize) -> usize;
        }
        // SIGPIPE is 13 and the default disposition is 0 on every unix here.
        signal(13, 0);
    }
}
