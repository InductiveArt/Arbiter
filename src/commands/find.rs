use std::fs;
use std::path::Path;
use std::process::ExitCode;

use crate::core::claims;
use crate::core::pack;
use crate::core::pack::Def;
use crate::core::pack::Edge;
use crate::core::pack::Finding;
use crate::laws::findings::drawn;
use crate::laws::findings::integrity;
use crate::laws::findings::winnow;
use crate::ledger;
use crate::packs::graphed;
use crate::records::brief;
use crate::rhyme;

// The one query door: a noun names a report, a rule names one law's instances,
// bare is every tool-free finding. A find reads and never writes.
pub fn run(rest: &[String]) -> ExitCode {
    let folder = |at: usize| rest.get(at).cloned().unwrap_or_else(|| ".".to_string());
    match rest.first().map(String::as_str) {
        Some("fallout") => {
            let Some(name) = rest.get(1) else {
                eprintln!("find fallout: name the symbol to look up");
                return ExitCode::from(2);
            };
            let asked = folder(2);
            let Some(found) = graphed(&asked) else {
                eprintln!("find: no folder at {asked}");
                return ExitCode::from(2);
            };
            fallout(name, &found.defs, &found.edges);
            ExitCode::SUCCESS
        }
        Some("deps") => {
            let Some(name) = rest.get(1) else {
                let Some(found) = graphed(".") else {
                    eprintln!("find: no folder at .");
                    return ExitCode::from(2);
                };
                return drawn_graph(&found);
            };
            let asked = folder(2);
            let Some(found) = graphed(&asked) else {
                eprintln!("find: no folder at {asked}");
                return ExitCode::from(2);
            };
            deps(name, &found)
        }
        Some("rhymes") => {
            let asked = folder(1);
            let Some(found) = graphed(&asked) else {
                eprintln!("find: no folder at {asked}");
                return ExitCode::from(2);
            };
            rhymed(&found)
        }
        Some("tangles") => {
            let scope = rest.get(1).cloned();
            let Some(found) = graphed(".") else {
                eprintln!("find: no folder at .");
                return ExitCode::from(2);
            };
            tangled(scope.as_deref(), &found)
        }
        Some("plan") => {
            let Some(found) = graphed(".") else {
                eprintln!("find: no folder at .");
                return ExitCode::from(2);
            };
            planned(rest.get(1).map(String::as_str), &found)
        }
        Some("impact") => {
            let Some(name) = rest.get(1) else {
                eprintln!("find impact: name the service to take down");
                return ExitCode::from(2);
            };
            let Some(found) = graphed(".") else {
                eprintln!("find: no folder at .");
                return ExitCode::from(2);
            };
            fallen(name, &found)
        }
        first => {
            let (rule, asked) = match first {
                None => (None, ".".to_string()),
                Some(word) if pack::findable(word) => (Some(word.to_string()), folder(1)),
                Some(word) if pack::drillable(word) => {
                    eprintln!(
                        "find: {word} fires through the gate's outcomes; \
                         run arbiter gate . and read .arbiter/last/report.md"
                    );
                    return ExitCode::from(2);
                }
                Some(word) if Path::new(word).is_dir() => (None, word.to_string()),
                Some(word) => {
                    eprintln!(
                        "find: nothing named {word}; find takes a rule from arbiter rules, \
                         fallout <name>, deps [unit], plan [name], impact <service>, rhymes or tangles"
                    );
                    return ExitCode::from(2);
                }
            };
            let Some(found) = graphed(&asked) else {
                eprintln!("find: no folder at {asked}");
                return ExitCode::from(2);
            };
            let commissioned = brief::commissioned(&found);
            let (winnowed, _) = winnow(&found.defs, &found.edges, &found.repo, &commissioned);
            let mut all = winnowed;
            all.extend(integrity(&found));
            all.extend(ledger::blanket_findings(&found));
            all.extend(ledger::broken_record(&found.root));
            all.extend(claims::vanished(&found));
            all.extend(brief::findings(&found));
            all.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
            emit(&all, rule.as_deref());
            ExitCode::SUCCESS
        }
    }
}

fn emit(findings: &[Finding], rule: Option<&str>) {
    for finding in findings {
        if rule.is_some_and(|rule| finding.name != rule) {
            continue;
        }
        let shown = finding.file.to_string_lossy();
        println!(
            "{}",
            serde_json::json!({
                "check": finding.check,
                "file": shown,
                "line": finding.line,
                "rule": finding.name,
                "reason": finding.reason,
            })
        );
    }
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

// The whole resolved unit graph as one mermaid fence, declared against real:
// an edge the declarations refuse is drawn dashed and named undeclared. A
// measure, never findings, so the fence pastes into any page the host draws.
fn drawn_graph(found: &crate::core::graph::Graph) -> ExitCode {
    let edges = drawn(found);
    let mut ids: std::collections::BTreeMap<&str, usize> = Default::default();
    for (user, owner, _) in &edges {
        let next = ids.len();
        ids.entry(user.as_str()).or_insert(next);
        let next = ids.len();
        ids.entry(owner.as_str()).or_insert(next);
    }
    println!("```mermaid");
    println!("flowchart TD");
    for (name, id) in &ids {
        println!("  u{id}[\"{name}\"]");
    }
    for (user, owner, breaks) in &edges {
        let from = ids[user.as_str()];
        let to = ids[owner.as_str()];
        if *breaks {
            println!("  u{from} -. undeclared .-> u{to}");
        } else {
            println!("  u{from} --> u{to}");
        }
    }
    if edges.is_empty() {
        println!("  %% no cross-unit edges resolved");
    }
    println!("  %% test units are not drawn");
    println!("```");
    ExitCode::SUCCESS
}

// One unit's whole dependency picture, every language in the one format: the
// written edges beside the observed ones, and the third parties its locks name.
fn deps(name: &str, found: &crate::core::graph::Graph) -> ExitCode {
    let shown_unit = |unit: usize| {
        let path = &found.repo.units[unit].path;
        if path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            path.to_string_lossy().to_string()
        }
    };
    let Some(unit) = (0..found.repo.units.len()).find(|&unit| shown_unit(unit) == *name) else {
        return scoped(name, found);
    };
    let mut uses = std::collections::BTreeSet::new();
    let mut users = std::collections::BTreeSet::new();
    for edge in &found.edges {
        let owner = found.defs[edge.def].unit;
        if owner == edge.unit {
            continue;
        }
        if edge.unit == unit {
            uses.insert(shown_unit(owner));
        }
        if owner == unit {
            users.insert(shown_unit(edge.unit));
        }
    }
    println!("deps: {name}");
    let listed = |set: &std::collections::BTreeSet<String>| {
        if set.is_empty() {
            "nothing".to_string()
        } else {
            set.iter().cloned().collect::<Vec<_>>().join(", ")
        }
    };
    println!("  uses: {}", listed(&uses));
    println!("  used by: {}", listed(&users));
    let declared = fs::read_to_string(found.root.join(".arbiter/units.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Value>().ok());
    let entry = declared
        .as_ref()
        .and_then(|parsed| parsed.get("units"))
        .and_then(|units| units.get(name));
    let names = |value: Option<&toml::Value>| -> Option<Vec<String>> {
        Some(
            value?
                .as_array()?
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect(),
        )
    };
    match names(entry.and_then(|entry| entry.get("deps"))) {
        Some(deps) => println!("  declares: {}", deps.join(", ")),
        None => println!("  declares: nothing"),
    }
    match names(entry.and_then(|entry| entry.get("visibility"))) {
        Some(grants) => println!("  grants: {}", grants.join(", ")),
        None => println!("  grants: everyone; no visibility is declared"),
    }
    for pack in &found.packs {
        let locks = pack.lockfiles();
        if locks.is_empty() {
            continue;
        }
        for path in &found.repo.lockfiles {
            let Some(name) = path.file_name().map(|name| name.to_string_lossy()) else {
                continue;
            };
            if !locks.contains(&name.as_ref()) {
                continue;
            }
            if found.repo.unit_of(path) != unit {
                continue;
            }
            let Ok(text) = fs::read_to_string(found.root.join(path)) else {
                continue;
            };
            let locked = pack.locked(&name, &text);
            println!(
                "  third parties ({}): {} locked",
                pack.label(),
                locked.len()
            );
            for (name, version) in locked {
                println!("    {name} {version}");
            }
        }
    }
    ExitCode::SUCCESS
}

// A folder or file inside a unit: the same read at its own grain — each file
// it reaches beyond itself, and each outside file standing on it.
fn scoped(asked: &str, found: &crate::core::graph::Graph) -> ExitCode {
    let scope = Path::new(asked.trim_start_matches("./"))
        .components()
        .collect::<std::path::PathBuf>();
    if scope.as_os_str().is_empty() || !found.root.join(&scope).exists() {
        eprintln!("deps: no unit or path named {asked}");
        return ExitCode::from(2);
    }
    let within = |file: &Path| file.starts_with(&scope);
    let mut uses = std::collections::BTreeSet::new();
    let mut users = std::collections::BTreeSet::new();
    for edge in &found.edges {
        let owner = &found.defs[edge.def].file;
        if within(&edge.file) && !within(owner) {
            uses.insert(owner.to_string_lossy().to_string());
        }
        if within(owner) && !within(&edge.file) {
            users.insert(edge.file.to_string_lossy().to_string());
        }
    }
    println!("deps: {}", scope.display());
    for (title, set) in [("uses", &uses), ("used by", &users)] {
        if set.is_empty() {
            println!("  {title}: nothing");
            continue;
        }
        println!("  {title}:");
        for file in set {
            println!("    {file}");
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
