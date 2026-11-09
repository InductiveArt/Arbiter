use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::core::graph::Graph;
use crate::core::pack::Finding;
use crate::core::units::fits;

pub const PLANS: &str = ".arbiter/plans.toml";

// The planning grain: deployment declared before it is possible. The record
// states services, the state they keep, the machines expected and the plans
// placing them; the laws hold it coherent, and no actuator is implied - any
// consumer of the record, or none, satisfies it.
pub struct Service {
    pub name: String,
    pub unit: Option<String>,
    pub needs: Vec<String>,
    pub channels: Vec<(String, Option<String>)>,
    pub accepts: Option<Vec<String>>,
    pub volumes: Vec<String>,
    pub cpu: Option<f64>,
    pub memory_mb: Option<i64>,
    // How many of this service a plan runs; absent is one.
    pub replicas: i64,
    // The doors: what this service serves to the world, routed to a declared
    // service. A domain may be a site-filled placeholder; no law reads it.
    pub serves: Vec<(String, i64, String)>,
    // The declared bridge from plan entities to the process: each value may
    // hold {port} or {volume:name}, resolved by whoever actuates a world.
    pub env: Vec<(String, String)>,
    pub line: usize,
}

pub struct Volume {
    pub name: String,
    pub backup: Option<String>,
    pub line: usize,
}

pub struct Machine {
    pub name: String,
    pub cpu: Option<f64>,
    pub memory_mb: Option<i64>,
}

pub struct Plan {
    pub name: String,
    pub place: Vec<(String, String)>,
    pub line: usize,
}

pub struct Reading {
    pub services: Vec<Service>,
    pub volumes: Vec<Volume>,
    pub machines: Vec<Machine>,
    pub plans: Vec<Plan>,
}

// The record is written once it holds a service; the seeded comments are not
// yet a record, the way an absent allowlist is not yet a license policy.
pub fn read(root: &Path) -> Option<Result<Reading, Finding>> {
    let text = fs::read_to_string(root.join(PLANS)).ok()?;
    let parsed = match text.parse::<toml::Value>() {
        Ok(parsed) => parsed,
        Err(_) => {
            return Some(Err(Finding {
                check: "plans",
                file: PathBuf::from(PLANS),
                line: 0,
                name: "unparsable-user-record".to_string(),
                reason: format!("{PLANS} does not parse; the plans cannot be read"),
            }));
        }
    };
    let line_of = |needle: &str| {
        text.lines()
            .position(|line| line.contains(needle))
            .map_or(0, |at| at + 1)
    };
    let word = |entry: &toml::Value, field: &str| {
        entry
            .get(field)
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    };
    let listed = |entry: &toml::Value, field: &str| -> Vec<String> {
        entry
            .get(field)
            .and_then(toml::Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let number = |entry: &toml::Value, field: &str| -> Option<f64> {
        entry.get(field).and_then(|value| {
            value
                .as_float()
                .or_else(|| value.as_integer().map(|whole| whole as f64))
        })
    };
    let entries = |name: &str| -> Vec<toml::Value> {
        parsed
            .get(name)
            .and_then(toml::Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let mut services = Vec::new();
    for entry in entries("service") {
        let Some(name) = word(&entry, "name") else {
            continue;
        };
        let channels = entry
            .get("channels")
            .and_then(toml::Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|channel| Some((word(channel, "to")?, word(channel, "contract"))))
                    .collect()
            })
            .unwrap_or_default();
        let accepts = entry
            .get("accepts")
            .and_then(toml::Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_string)
                    .collect()
            });
        let serves = entry
            .get("serves")
            .and_then(toml::Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|door| {
                        Some((
                            word(door, "domain")?,
                            door.get("port")
                                .and_then(toml::Value::as_integer)
                                .unwrap_or(443),
                            word(door, "to")?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let env = entry
            .get("env")
            .and_then(toml::Value::as_table)
            .map(|table| {
                table
                    .iter()
                    .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        services.push(Service {
            line: line_of(&format!("\"{name}\"")),
            unit: word(&entry, "unit"),
            needs: listed(&entry, "needs"),
            channels,
            accepts,
            env,
            volumes: listed(&entry, "volumes"),
            cpu: number(&entry, "cpu"),
            memory_mb: number(&entry, "memory_mb").map(|value| value as i64),
            replicas: number(&entry, "replicas")
                .map(|value| value as i64)
                .unwrap_or(1),
            serves,
            name,
        });
    }
    if services.is_empty() {
        return None;
    }
    let mut volumes = Vec::new();
    for entry in entries("volume") {
        let Some(name) = word(&entry, "name") else {
            continue;
        };
        volumes.push(Volume {
            line: line_of(&format!("\"{name}\"")),
            backup: word(&entry, "backup"),
            name,
        });
    }
    let mut machines = Vec::new();
    for entry in entries("machine") {
        let Some(name) = word(&entry, "name") else {
            continue;
        };
        machines.push(Machine {
            cpu: number(&entry, "cpu"),
            memory_mb: number(&entry, "memory_mb").map(|value| value as i64),
            name,
        });
    }
    let mut plans = Vec::new();
    for entry in entries("plan") {
        let Some(name) = word(&entry, "name") else {
            continue;
        };
        let place = entry
            .get("place")
            .and_then(toml::Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|placement| {
                        Some((word(placement, "service")?, word(placement, "on")?))
                    })
                    .collect()
            })
            .unwrap_or_default();
        plans.push(Plan {
            line: line_of(&format!("\"{name}\"")),
            place,
            name,
        });
    }
    Some(Ok(Reading {
        services,
        volumes,
        machines,
        plans,
    }))
}

// The nine laws over one reading. Absent pieces bind nothing: a service
// without a unit stays open, a record without plans places no one, a service
// without accepts invites everyone.
pub fn findings(found: &Graph) -> Vec<Finding> {
    let place = PathBuf::from(PLANS);
    let reading = match read(&found.root) {
        None => return Vec::new(),
        Some(Err(broken)) => return vec![broken],
        Some(Ok(reading)) => reading,
    };
    let mut findings = Vec::new();
    let at = |line: usize, name: &str, reason: String| Finding {
        check: "plans",
        file: place.clone(),
        line,
        name: name.to_string(),
        reason,
    };
    let services: BTreeMap<&str, &Service> = reading
        .services
        .iter()
        .map(|service| (service.name.as_str(), service))
        .collect();
    // Every dialed name resolves: needs, channels and placements.
    for service in &reading.services {
        for need in &service.needs {
            if !services.contains_key(need.as_str()) {
                findings.push(at(
                    service.line,
                    "phantom-service",
                    format!("{} needs {need}, which no service declares", service.name),
                ));
            }
        }
        for (to, _) in &service.channels {
            if !services.contains_key(to.as_str()) {
                findings.push(at(
                    service.line,
                    "phantom-service",
                    format!("{} dials {to}, which no service declares", service.name),
                ));
            }
        }
        for volume in &service.volumes {
            if !reading.volumes.iter().any(|known| known.name == *volume) {
                findings.push(at(
                    service.line,
                    "undeclared-volume",
                    format!(
                        "{} mounts {volume}, which the record does not declare",
                        service.name
                    ),
                ));
            }
        }
        for (_, _, to) in &service.serves {
            if !services.contains_key(to.as_str()) {
                findings.push(at(
                    service.line,
                    "phantom-service",
                    format!("{} serves {to}, which no service declares", service.name),
                ));
            }
        }
        if let Some(unit) = &service.unit {
            let built =
                (0..found.repo.units.len()).any(|index| found.repo.unit_name(index) == *unit);
            if !built {
                findings.push(at(
                    service.line,
                    "unbuilt-artifact",
                    format!(
                        "{} names unit {unit}, which this repository does not build",
                        service.name
                    ),
                ));
            }
        }
    }
    for volume in &reading.volumes {
        if volume.backup.is_none() {
            findings.push(at(
                volume.line,
                "unbacked-volume",
                format!(
                    "{} names no backup policy; state is never implicit",
                    volume.name
                ),
            ));
        }
    }
    // Bring-up is construction: the needs form an order.
    let names: Vec<&str> = reading
        .services
        .iter()
        .map(|service| service.name.as_str())
        .collect();
    let index: BTreeMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(position, name)| (*name, position))
        .collect();
    let mut adjacency = vec![BTreeSet::new(); names.len()];
    for service in &reading.services {
        for need in &service.needs {
            if let (Some(&from), Some(&to)) =
                (index.get(service.name.as_str()), index.get(need.as_str()))
            {
                adjacency[from].insert(to);
            }
        }
    }
    for group in crate::core::graph::strongly_connected(&adjacency) {
        if group.len() < 2 {
            continue;
        }
        let mut members: Vec<&str> = group.iter().map(|&position| names[position]).collect();
        members.sort_unstable();
        findings.push(at(
            0,
            "boot-cycle",
            format!(
                "the needs of {} form a cycle; this plan cannot boot",
                members.join(" <-> ")
            ),
        ));
    }
    // Channels carry the called service's consent.
    let mut granted: BTreeSet<(String, usize)> = BTreeSet::new();
    for service in &reading.services {
        for (to, _) in &service.channels {
            let Some(called) = services.get(to.as_str()) else {
                continue;
            };
            let Some(accepts) = &called.accepts else {
                continue;
            };
            let mut invited = false;
            for (entry, pattern) in accepts.iter().enumerate() {
                if fits(pattern, &service.name) {
                    granted.insert((called.name.clone(), entry));
                    invited = true;
                }
            }
            if !invited {
                findings.push(at(
                    service.line,
                    "uninvited-channel",
                    format!(
                        "{} dials {to}, which only {} may",
                        service.name,
                        accepts.join(", ")
                    ),
                ));
            }
        }
    }
    for service in &reading.services {
        let Some(accepts) = &service.accepts else {
            continue;
        };
        for (entry, pattern) in accepts.iter().enumerate() {
            if pattern.contains('*') || granted.contains(&(service.name.clone(), entry)) {
                continue;
            }
            findings.push(at(
                service.line,
                "stale-channel-grant",
                format!("{} grants {pattern}, which never dials it", service.name),
            ));
        }
    }
    // Once a plan is written, every service is placed and every machine fits.
    if !reading.plans.is_empty() {
        for service in &reading.services {
            let placed = reading
                .plans
                .iter()
                .any(|plan| plan.place.iter().any(|(name, _)| name == &service.name));
            if !placed {
                findings.push(at(
                    service.line,
                    "unplanned-service",
                    format!("{} is placed by no plan", service.name),
                ));
            }
        }
        for plan in &reading.plans {
            let mut loads: BTreeMap<&str, (f64, i64)> = BTreeMap::new();
            for (name, machine) in &plan.place {
                let Some(service) = services.get(name.as_str()) else {
                    findings.push(at(
                        plan.line,
                        "phantom-service",
                        format!(
                            "plan {} places {name}, which no service declares",
                            plan.name
                        ),
                    ));
                    continue;
                };
                let load = loads.entry(machine.as_str()).or_insert((0.0, 0));
                let count = service.replicas.max(1) as f64;
                load.0 += service.cpu.unwrap_or(0.0) * count;
                load.1 += service.memory_mb.unwrap_or(0) * service.replicas.max(1);
            }
            for (machine, (cpu, memory)) in loads {
                let Some(known) = reading.machines.iter().find(|known| known.name == machine)
                else {
                    continue;
                };
                let over_cpu = known.cpu.is_some_and(|capacity| cpu > capacity);
                let over_memory = known.memory_mb.is_some_and(|capacity| memory > capacity);
                if over_cpu || over_memory {
                    findings.push(at(
                        plan.line,
                        "overcommitted-machine",
                        format!(
                            "plan {} loads {machine} past its declared capacity",
                            plan.name
                        ),
                    ));
                }
            }
        }
    }
    findings
}

// The bring-up ladder: the placed services of one plan, in the order their
// needs allow, siblings of one rung starting together.
// Everything a service stands on, transitively: its boot needs and the
// services it dials at runtime.
pub fn stands_on(reading: &Reading, name: &str) -> BTreeSet<String> {
    let mut cone = BTreeSet::new();
    let mut frontier = vec![name.to_string()];
    while let Some(place) = frontier.pop() {
        let Some(service) = reading.services.iter().find(|one| one.name == place) else {
            continue;
        };
        for below in service
            .needs
            .iter()
            .chain(service.channels.iter().map(|(to, _)| to))
        {
            if cone.insert(below.clone()) {
                frontier.push(below.clone());
            }
        }
    }
    cone
}

// Every service that falls when this one is down: the reverse of stands_on.
pub fn impacted(reading: &Reading, name: &str) -> Vec<String> {
    reading
        .services
        .iter()
        .filter(|service| service.name != name && stands_on(reading, &service.name).contains(name))
        .map(|service| service.name.clone())
        .collect()
}

pub fn ladder(reading: &Reading, plan: &Plan) -> Vec<Vec<(String, String)>> {
    let placed: BTreeMap<&str, &str> = plan
        .place
        .iter()
        .map(|(service, machine)| (service.as_str(), machine.as_str()))
        .collect();
    let mut remaining: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for service in &reading.services {
        if !placed.contains_key(service.name.as_str()) {
            continue;
        }
        let needs: BTreeSet<&str> = service
            .needs
            .iter()
            .map(String::as_str)
            .filter(|need| placed.contains_key(need))
            .collect();
        remaining.insert(service.name.as_str(), needs);
    }
    let mut rungs = Vec::new();
    while !remaining.is_empty() {
        let ready: Vec<&str> = remaining
            .iter()
            .filter(|(_, needs)| needs.is_empty())
            .map(|(name, _)| *name)
            .collect();
        if ready.is_empty() {
            // A cycle among the placed: the boot-cycle law names it; the
            // ladder shows what remains, unordered, rather than looping.
            let stuck: Vec<(String, String)> = remaining
                .keys()
                .map(|name| (name.to_string(), placed[name].to_string()))
                .collect();
            rungs.push(stuck);
            break;
        }
        rungs.push(
            ready
                .iter()
                .map(|name| (name.to_string(), placed[name].to_string()))
                .collect(),
        );
        for name in &ready {
            remaining.remove(name);
        }
        for needs in remaining.values_mut() {
            for name in &ready {
                needs.remove(name);
            }
        }
    }
    rungs
}
