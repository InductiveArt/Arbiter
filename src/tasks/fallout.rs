use std::process::ExitCode;

use crate::core::pack::Def;
use crate::core::pack::Edge;
use crate::core::task::Task;
use crate::packs::graphed;

pub fn task() -> Task {
    Task {
        name: "fallout",
        line: "every definition of a name and each resolved use",
        run: query,
    }
}

fn query(rest: &[String]) -> ExitCode {
    let Some(name) = rest.first() else {
        eprintln!("find fallout: name the symbol to look up");
        return ExitCode::from(2);
    };
    let asked = rest.get(1).cloned().unwrap_or_else(|| ".".to_string());
    let Some(found) = graphed(&asked) else {
        eprintln!("find: no folder at {asked}");
        return ExitCode::from(2);
    };
    fallout(name, &found.defs, &found.edges);
    ExitCode::SUCCESS
}

fn fallout(name: &str, defs: &[Def], edges: &[Edge]) {
    let matched: Vec<(usize, &Def)> = defs
        .iter()
        .enumerate()
        .filter(|(_, def)| def.name == name)
        .collect();
    if matched.is_empty() {
        println!("fallout: no public symbol named {name}");
        return;
    }
    for (index, def) in matched {
        println!("{}:{} {}", def.file.to_string_lossy(), def.line, def.name);
        let mut users: Vec<String> = edges
            .iter()
            .filter(|edge| edge.def == index && edge.file != def.file)
            .map(|edge| format!("  used by {}:{}", edge.file.to_string_lossy(), edge.line))
            .collect();
        users.sort();
        users.dedup();
        if def.self_used {
            println!("  used by its own file");
        } else if users.is_empty() {
            println!("  used by nothing");
        }
        for user in users {
            println!("{user}");
        }
    }
}
