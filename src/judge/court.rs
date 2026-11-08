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
            serde_json::json!({
                "unit": outcome.unit,
                "check": outcome.check,
                "passed": outcome.passed,
                "warm": outcome.warm,
                "seconds": outcome.seconds,
            })
        })
        .collect();
    let found_findings: Vec<serde_json::Value> = run
        .findings
        .iter()
        .map(|finding| {
            serde_json::json!({
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
                serde_json::json!({ "case": need.case, "need": need.need, "state": state })
            })
            .collect();
        serde_json::json!({ "cases": reading.cases, "claims": reading.claims, "needs": needs })
    });
    let counted: Vec<String> = ledger::counted(found)
        .iter()
        .map(|suppression| {
            format!(
                "{}:{}  {}",
                suppression.file.to_string_lossy(),
                suppression.line,
                suppression.text
            )
        })
        .collect();
    serde_json::json!({
        "version": crate::verbs::VERSION,
        "root": run.root.to_string_lossy(),
        "units": run.units,
        "files": run.files,
        "outcomes": outcomes,
        "findings": found_findings,
        "rules": rules,
        "edges": edges,
        "tangles": tangles,
        "plans": plans,
        "brief": brief,
        "suppressions": { "counted": counted, "added": run.added },
        "third": { "locked": run.third, "added": run.third_added },
        "claims": run.claims,
    })
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
    serde_json::json!({
        "services": services,
        "plans": plans,
        "doors": doors,
        "channels": channels,
    })
}
