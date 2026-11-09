use std::fs;
use std::path::PathBuf;

use crate::core::pack;
use crate::judge::brief;
use crate::judge::findings;
use crate::judge::ledger;
use crate::judge::plans;
use crate::judge::verdict::Run;

const TEMPLATE: &str = include_str!("court.html");

// The court: the run rendered whole as one page and one snapshot, written as
// evidence beside the report. The page is self-contained - the snapshot is
// embedded, nothing is fetched - and the snapshot alone is the contract any
// other console may draw from.
pub fn write(run: &Run) -> Option<PathBuf> {
    let folder = run.root.join(".arbiter").join("last");
    fs::create_dir_all(&folder).ok()?;
    let data = snapshot(run);
    fs::write(
        folder.join("graph.json"),
        serde_json::to_string_pretty(&data).ok()? + "\n",
    )
    .ok()?;
    let place = folder.join("court.html");
    fs::write(&place, page(&data)).ok()?;
    Some(place)
}

// The embedded form: `</` is split so no reason can close the script early.
pub fn page(data: &serde_json::Value) -> String {
    let embedded = data.to_string().replace("</", "<\\/");
    TEMPLATE.replace("/*ARBITER_DATA*/", &embedded)
}

fn snapshot(run: &Run) -> serde_json::Value {
    let found = &run.graph;
    let outcomes: Vec<serde_json::Value> = run
        .outcomes
        .iter()
        .map(|outcome| {
            let mut output = outcome.output.clone();
            if output.len() > 30_000 {
                output.truncate(30_000);
                output.push_str("\n… truncated; the whole output is in the report");
            }
            serde_json::json!({
                "unit": if outcome.unit.is_empty() { "." } else { &outcome.unit },
                "pack": outcome.pack,
                "check": outcome.check,
                "passed": outcome.passed,
                "warm": outcome.warm,
                "seconds": outcome.seconds,
                "output": output,
            })
        })
        .collect();
    let adapters: Vec<serde_json::Value> = crate::packs::all()
        .iter()
        .filter_map(|pack| {
            let adapter = pack.adapter()?;
            let parsed = adapter.parse::<toml::Value>().ok()?;
            let checks: Vec<String> = parsed
                .get("check")?
                .as_array()?
                .iter()
                .filter_map(|check| check.get("name").and_then(toml::Value::as_str))
                .map(str::to_string)
                .collect();
            Some(serde_json::json!({ "pack": pack.label(), "checks": checks }))
        })
        .collect();
    let units_list: Vec<serde_json::Value> = (0..found.repo.units.len())
        .map(|index| {
            let home = found.repo.units[index].path.clone();
            serde_json::json!({
                "name": found.repo.unit_name(index),
                "test": found.repo.is_test(index, &home),
            })
        })
        .collect();
    let found_findings: Vec<serde_json::Value> = run
        .findings
        .iter()
        .map(|finding| {
            // The id hashes what the finding is, never where its line drifted:
            // the same offense yields the same id run after run.
            let mut digest = crate::core::digest::Digest::new();
            digest.eat(finding.name.as_bytes());
            digest.eat(&[0]);
            digest.eat(finding.file.to_string_lossy().as_bytes());
            digest.eat(&[0]);
            digest.eat(finding.reason.as_bytes());
            serde_json::json!({
                "id": digest.hex()[..12].to_string(),
                "check": finding.check,
                "file": finding.file.to_string_lossy(),
                "line": finding.line,
                "rule": finding.name,
                "reason": finding.reason,
            })
        })
        .collect();
    let rules: Vec<serde_json::Value> = pack::GROUPS
        .iter()
        .flat_map(|(group, members)| {
            members.iter().map(move |member| {
                serde_json::json!({
                    "name": member,
                    "law": pack::law(member).unwrap_or(""),
                    "group": group,
                    "core": pack::CORE.iter().any(|(name, _)| name == member),
                    "record": matches!(pack::power(member), pack::Power::Record(_)),
                })
            })
        })
        .collect();
    let edges: Vec<serde_json::Value> = findings::drawn(found)
        .into_iter()
        .map(|(user, owner, breaks)| {
            serde_json::json!({ "user": user, "owner": owner, "breaks": breaks })
        })
        .collect();
    let families: Vec<serde_json::Value> = findings::families(found)
        .into_iter()
        .map(|family| {
            let drawn: Vec<serde_json::Value> = family
                .edges
                .into_iter()
                .map(|(from, to)| serde_json::json!({ "from": from, "to": to }))
                .collect();
            serde_json::json!({ "unit": family.unit, "place": family.place, "edges": drawn })
        })
        .collect();
    let declared = declared_units(run);
    let tangles: Vec<serde_json::Value> = findings::tangles(found)
        .into_iter()
        .map(|(place, members)| serde_json::json!({ "place": place, "members": members }))
        .collect();
    let plans = plans_snapshot(run);
    let brief = brief::read(found).map(|reading| {
        let needs: Vec<serde_json::Value> = reading
            .needs
            .iter()
            .map(|need| {
                let state = match need.state {
                    brief::State::Held => "held",
                    brief::State::Planned => "planned",
                    brief::State::Manual => "manual",
                    brief::State::Unanswered => "unanswered",
                };
                serde_json::json!({
                    "case": need.case,
                    "need": need.need,
                    "state": state,
                    "claims": need.entries.len(),
                })
            })
            .collect();
        let seated: Vec<serde_json::Value> = reading
            .cases
            .iter()
            .map(|case| {
                serde_json::json!({
                    "name": case.name,
                    "unit": case.unit,
                    "serves": case.serves,
                })
            })
            .collect();
        serde_json::json!({
            "cases": reading.cases.len(),
            "seated": seated,
            "claims": reading.claims,
            "needs": needs,
        })
    });
    let receipts = fs::read_to_string(run.root.join(".arbiter/last/receipts.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let counted: Vec<serde_json::Value> = ledger::counted(found)
        .iter()
        .map(|suppression| {
            serde_json::json!({
                "file": suppression.file.to_string_lossy(),
                "line": suppression.line,
                "text": suppression.text,
            })
        })
        .collect();
    serde_json::json!({
        "version": crate::verbs::VERSION,
        "root": run.root.to_string_lossy(),
        "units": run.units,
        "files": run.files,
        "foreign": found.repo.foreign.iter().map(|place| {
            serde_json::Value::String(place.to_string_lossy().to_string())
        }).collect::<Vec<serde_json::Value>>(),
        "outcomes": outcomes,
        "adapters": adapters,
        "resolved": found.edges.len(),
        "pipeline": run.steps.iter().map(|step| {
            serde_json::json!({ "name": step.name, "seconds": step.seconds })
        }).collect::<Vec<serde_json::Value>>(),
        "unitsList": units_list,
        "findings": found_findings,
        "rules": rules,
        "edges": edges,
        "families": families,
        "declared": declared,
        "tangles": tangles,
        "plans": plans,
        "brief": brief,
        "receipts": receipts,
        "suppressions": { "counted": counted, "added": run.added },
        "third": { "locked": run.third, "added": run.third_added },
        "claims": run.claims,
    })
}

// The written unit declarations, so the panel shows intent beside the real.
fn declared_units(run: &Run) -> Vec<serde_json::Value> {
    let Ok(text) = fs::read_to_string(run.root.join(crate::core::units::DECLARATIONS)) else {
        return Vec::new();
    };
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return Vec::new();
    };
    let Some(units) = parsed.get("units").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let listed = |value: &toml::Value, field: &str| -> Vec<String> {
        value
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
    units
        .iter()
        .map(|(name, value)| {
            let inner: Vec<serde_json::Value> = value
                .get("inner")
                .and_then(toml::Value::as_table)
                .map(|table| {
                    table
                        .iter()
                        .map(|(path, node)| {
                            serde_json::json!({
                                "path": path,
                                "deps": listed(node, "deps"),
                                "visibility": listed(node, "visibility"),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            serde_json::json!({
                "name": name,
                "deps": listed(value, "deps"),
                "visibility": listed(value, "visibility"),
                "inner": inner,
            })
        })
        .collect()
}

fn plans_snapshot(run: &Run) -> serde_json::Value {
    let Some(Ok(reading)) = plans::read(&run.root) else {
        return serde_json::json!({ "services": [], "plans": [], "doors": [], "channels": [] });
    };
    let services: Vec<serde_json::Value> = reading
        .services
        .iter()
        .map(|service| {
            serde_json::json!({
                "name": service.name,
                "unit": service.unit,
                "replicas": service.replicas,
                "needs": service.needs,
                "volumes": service.volumes,
                "cpu": service.cpu,
                "memory_mb": service.memory_mb,
                "accepts": service.accepts,
                "line": service.line,
            })
        })
        .collect();
    let replicas_of = |name: &str| {
        reading
            .services
            .iter()
            .find(|service| service.name == name)
            .map_or(1, |service| service.replicas.max(1))
    };
    let plans: Vec<serde_json::Value> = reading
        .plans
        .iter()
        .map(|plan| {
            let ladder: Vec<Vec<serde_json::Value>> = plans::ladder(&reading, plan)
                .into_iter()
                .map(|rung| {
                    rung.into_iter()
                        .map(|(service, machine)| {
                            serde_json::json!({
                                "service": service,
                                "machine": machine,
                                "replicas": replicas_of(&service),
                            })
                        })
                        .collect()
                })
                .collect();
            serde_json::json!({ "name": plan.name, "ladder": ladder })
        })
        .collect();
    let mut doors = Vec::new();
    let mut channels = Vec::new();
    for service in &reading.services {
        for (domain, port, to) in &service.serves {
            doors.push(serde_json::json!({
                "domain": domain, "port": port, "to": to, "via": service.name,
            }));
        }
        for (to, contract) in &service.channels {
            channels.push(serde_json::json!({
                "from": service.name, "to": to, "contract": contract,
            }));
        }
    }
    let machines: Vec<serde_json::Value> = reading
        .machines
        .iter()
        .map(|machine| {
            serde_json::json!({
                "name": machine.name,
                "cpu": machine.cpu,
                "memory_mb": machine.memory_mb,
            })
        })
        .collect();
    let volumes: Vec<serde_json::Value> = reading
        .volumes
        .iter()
        .map(|volume| serde_json::json!({ "name": volume.name, "backup": volume.backup }))
        .collect();
    serde_json::json!({
        "services": services,
        "plans": plans,
        "doors": doors,
        "channels": channels,
        "machines": machines,
        "volumes": volumes,
    })
}
