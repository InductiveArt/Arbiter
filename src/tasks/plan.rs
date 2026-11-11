use std::process::ExitCode;

use crate::core::task::Task;
use crate::packs::graphed;

pub fn task() -> Task {
    Task {
        name: "plan",
        line: "a plan's bring-up ladder and channels; bare, every plan",
        run: query,
    }
}

fn query(rest: &[String]) -> ExitCode {
    let Some(found) = graphed(".") else {
        eprintln!("find: no folder at .");
        return ExitCode::from(2);
    };
    planned(rest.first().map(String::as_str), &found)
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
