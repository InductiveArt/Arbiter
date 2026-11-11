use std::process::ExitCode;

use crate::core::task::Task;
use crate::packs::graphed;

// Entangled siblings, questioned: the map to read before representing.
pub fn task() -> Task {
    Task {
        name: "tangles",
        line: "entangled siblings per folder, largest first; a measure, never findings",
        run: query,
    }
}

fn query(rest: &[String]) -> ExitCode {
    let scope = rest.first().cloned();
    let Some(found) = graphed(".") else {
        eprintln!("find: no folder at .");
        return ExitCode::from(2);
    };
    tangled(scope.as_deref(), &found)
}

// Every sibling cycle the facts hold, suppressed or not, largest first: the
// map an agent reads before representing - a measure, never findings.
fn tangled(scope: Option<&str>, found: &crate::core::graph::Graph) -> ExitCode {
    let groups = crate::laws::findings::tangles(found);
    let mut shown = 0;
    for (place, members) in &groups {
        if scope.is_some_and(|asked| !(place == asked || place.starts_with(&format!("{asked}/")))) {
            continue;
        }
        println!("{place}: {}", members.join(" <-> "));
        shown += 1;
    }
    if shown == 0 {
        println!("tangles: none");
    }
    ExitCode::SUCCESS
}
