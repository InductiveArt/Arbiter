use std::process::ExitCode;

use crate::core::task::Task;
use crate::packs::graphed;
use crate::records::brief;

// The declared deployment, questioned: a plan's bring-up, and what falls with
// one service.
pub fn tasks() -> Vec<Task> {
    vec![
        Task {
            name: "plan",
            line: "a plan's bring-up ladder and channels; bare, every plan",
            run: plan_query,
        },
        Task {
            name: "impact",
            line: "everything that falls with one service, and in what capacity",
            run: impact_query,
        },
    ]
}

fn plan_query(rest: &[String]) -> ExitCode {
    let Some(found) = graphed(".") else {
        eprintln!("find: no folder at .");
        return ExitCode::from(2);
    };
    planned(rest.first().map(String::as_str), &found)
}

fn impact_query(rest: &[String]) -> ExitCode {
    let Some(name) = rest.first() else {
        eprintln!("find impact: name the service to take down");
        return ExitCode::from(2);
    };
    let Some(found) = graphed(".") else {
        eprintln!("find: no folder at .");
        return ExitCode::from(2);
    };
    fallen(name, &found)
}

// One plan's bring-up as a ladder - rung by rung, nothing running - and its
// channels; bare, every plan the record holds. The view of the desired side.
fn planned(asked: Option<&str>, found: &crate::core::graph::Graph) -> ExitCode {
    let reading = match crate::records::plans::read(&found.root) {
        None => {
            println!("plans: none written");
            return ExitCode::SUCCESS;
        }
        Some(Err(broken)) => {
            eprintln!("find plan: {}", broken.reason);
            return ExitCode::from(2);
        }
        Some(Ok(reading)) => reading,
    };
    let mut shown = 0;
    for plan in &reading.plans {
        if asked.is_some_and(|name| name != plan.name) {
            continue;
        }
        shown += 1;
        println!("plan {}", plan.name);
        let spoken = |name: &str| -> String {
            let count = reading
                .services
                .iter()
                .find(|service| service.name == name)
                .map_or(1, |service| service.replicas.max(1));
            if count > 1 {
                format!("{name} x{count}")
            } else {
                name.to_string()
            }
        };
        let rungs = crate::records::plans::ladder(&reading, plan);
        for (rung, services) in rungs.iter().enumerate() {
            let step: Vec<String> = services
                .iter()
                .map(|(service, machine)| format!("{} on {machine}", spoken(service)))
                .collect();
            println!("  up   {}  {}", rung + 1, step.join(", "));
        }
        for (rung, services) in rungs.iter().enumerate().rev() {
            let step: Vec<String> = services
                .iter()
                .map(|(service, _)| spoken(service))
                .collect();
            println!("  down {}  {}", rungs.len() - rung, step.join(", "));
        }
        for service in &reading.services {
            for (domain, port, to) in &service.serves {
                println!("  door {domain}:{port} -> {to}  via {}", service.name);
            }
        }
        for service in &reading.services {
            for (to, contract) in &service.channels {
                match contract {
                    Some(contract) => {
                        println!("  channel {} -> {to}  {contract}", service.name);
                    }
                    None => println!("  channel {} -> {to}", service.name),
                }
            }
        }
    }
    if shown == 0 {
        match asked {
            Some(name) => {
                eprintln!("find plan: no plan named {name}");
                return ExitCode::from(2);
            }
            None => println!("plans: services declared, no plan written yet"),
        }
    }
    ExitCode::SUCCESS
}

// What falls when one service is down, and in what capacity: who cannot
// boot, who degrades, which doors go dark, what state turns unreachable,
// and which cases are placed over it.
fn fallen(name: &str, found: &crate::core::graph::Graph) -> ExitCode {
    let Some(Ok(reading)) = crate::records::plans::read(&found.root) else {
        eprintln!("find impact: no plans record; impact runs over the declared world");
        return ExitCode::from(2);
    };
    if !reading.services.iter().any(|service| service.name == *name) {
        eprintln!("find impact: no service named {name} in the plans");
        return ExitCode::from(2);
    }
    println!("if {name} is down:");
    let (boot, dials) = crate::records::plans::leaning_on(&reading, name);
    for service in &boot {
        println!("  cannot boot: {service} — needs it");
    }
    for (service, contract) in &dials {
        match contract {
            Some(contract) => println!("  degraded: {service} — dials it ({contract})"),
            None => println!("  degraded: {service} — dials it"),
        }
    }
    let direct: std::collections::BTreeSet<&String> = boot
        .iter()
        .chain(dials.iter().map(|(who, _)| who))
        .collect();
    let beyond: Vec<String> = crate::records::plans::impacted(&reading, name)
        .into_iter()
        .filter(|service| !direct.contains(service))
        .collect();
    if !beyond.is_empty() {
        println!("  and standing above those: {}", beyond.join(", "));
    }
    if direct.is_empty() && beyond.is_empty() {
        println!("  no service stands on it");
    }
    let mut doors = Vec::new();
    for service in &reading.services {
        for (domain, port, to) in &service.serves {
            if to == name || service.name == *name {
                doors.push(format!("{domain}:{port}"));
            }
        }
    }
    doors.sort();
    doors.dedup();
    if !doors.is_empty() {
        println!("  dark doors: {}", doors.join(", "));
    }
    let fallen_service = reading
        .services
        .iter()
        .find(|service| service.name == *name);
    if let Some(service) = fallen_service {
        let orphaned: Vec<&String> = service
            .volumes
            .iter()
            .filter(|volume| {
                !reading
                    .services
                    .iter()
                    .any(|other| other.name != *name && other.volumes.contains(volume))
            })
            .collect();
        if !orphaned.is_empty() {
            println!(
                "  unreachable state: {}",
                orphaned
                    .iter()
                    .map(|volume| volume.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
            );
        }
    }
    let mut placed: Vec<String> = Vec::new();
    if let Some(brief) = brief::read(found) {
        for case in &brief.cases {
            let Some(unit) = &case.unit else { continue };
            let mut cone: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            for service in reading
                .services
                .iter()
                .filter(|one| one.unit.as_deref() == Some(unit.as_str()))
            {
                cone.insert(service.name.clone());
                cone.extend(crate::records::plans::stands_on(&reading, &service.name));
            }
            if cone.contains(name) {
                placed.push(case.name.clone());
            }
        }
    }
    let mut cases: Vec<String> = Vec::new();
    for case in &placed {
        cases.push(format!("{case} — placed over it"));
    }
    cases.sort();
    cases.dedup();
    if cases.is_empty() {
        println!("  cases: none placed over it");
    } else {
        for case in cases {
            println!("  case: {case}");
        }
    }
    ExitCode::SUCCESS
}
