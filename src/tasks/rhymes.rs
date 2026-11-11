use std::process::ExitCode;

use crate::core::task::Task;
use crate::packs::graphed;
use crate::rhyme;

// Structural equivalence, questioned: the shapes that sound in more than one
// place.
pub fn task() -> Task {
    Task {
        name: "rhymes",
        line: "structurally equal functions, largest first; a measure, never findings",
        run: query,
    }
}

fn query(rest: &[String]) -> ExitCode {
    let asked = rest.first().cloned().unwrap_or_else(|| ".".to_string());
    let Some(found) = graphed(&asked) else {
        eprintln!("find: no folder at {asked}");
        return ExitCode::from(2);
    };
    rhymed(&found)
}

fn rhymed(found: &crate::core::graph::Graph) -> ExitCode {
    let groups = rhyme::rhymes(found);
    if groups.is_empty() {
        println!("rhymes: nothing repeats at function grain");
        return ExitCode::SUCCESS;
    }
    let plural = if groups.len() == 1 { "" } else { "s" };
    println!(
        "rhymes: {} shape{plural} sounding in more than one place",
        groups.len()
    );
    for group in groups {
        let kind = if group.perfect { "perfect" } else { "slant" };
        println!();
        println!(
            "{kind}, {} places, {} leaves:",
            group.places.len(),
            group.leaves
        );
        for place in group.places {
            let title = if place.title.is_empty() {
                String::new()
            } else {
                format!(" {}", place.title)
            };
            println!("  {}:{}{}", place.file.to_string_lossy(), place.line, title);
        }
    }
    ExitCode::SUCCESS
}
