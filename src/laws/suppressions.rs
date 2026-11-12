use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::graph::strongly_connected;
use crate::core::graph::Graph;
use crate::core::measures::sibling_graphs;
use crate::core::pack::Finding;
use crate::core::task::Law;

pub fn suppressions_law() -> Law {
    Law {
        task: "suppressions",
        run: judge,
    }
}

// Everything the suppression record governs: its guards, the sibling cycles it
// quiets, the grants gone stale, a suppression naming a language no pack judges,
// the blanket forms, and a record that will not parse.
fn judge(found: &Graph) -> Vec<Finding> {
    let mut findings = Vec::new();
    let (subjects, guards) = crate::core::record::read(&found.root);
    findings.extend(guards);
    let (cycles_found, cycles_live) = sibling_cycles(found, &subjects);
    findings.extend(cycles_found);
    findings.extend(crate::core::record::stale_cycles(&subjects, &cycles_live));
    for subject in &subjects {
        let crate::core::record::Subject::Dependency {
            language,
            package,
            line,
            ..
        } = subject
        else {
            continue;
        };
        if !found.packs.iter().any(|pack| pack.label() == language) {
            findings.push(Finding {
                check: "integrity",
                file: PathBuf::from(crate::core::record::SUPPRESSIONS),
                line: *line,
                name: "stale-suppression".to_string(),
                reason: format!("{package} is suppressed for {language}, which no pack judges"),
            });
        }
    }
    findings.extend(crate::ledger::blanket_findings(found));
    findings.extend(crate::ledger::broken_record(&found.root));
    findings
}

// The sibling law: within every folder, the children form an order. A cycle
// is named with its whole member set; the user's record may quiet one exact
// set, and a quieting entry is proven alive or named stale.
fn sibling_cycles(
    found: &Graph,
    subjects: &[crate::core::record::Subject],
) -> (Vec<Finding>, std::collections::HashSet<usize>) {
    let mut findings = Vec::new();
    let mut live = std::collections::HashSet::new();
    for ((unit, folder), children) in sibling_graphs(found) {
        let names: Vec<&String> = children.keys().collect();
        let index: std::collections::BTreeMap<&str, usize> = names
            .iter()
            .enumerate()
            .map(|(at, name)| (name.as_str(), at))
            .collect();
        let mut adjacency = vec![BTreeSet::new(); names.len()];
        for (from, reached) in &children {
            for to in reached {
                if let (Some(&a), Some(&b)) = (index.get(from.as_str()), index.get(to.as_str())) {
                    adjacency[a].insert(b);
                }
            }
        }
        for group in strongly_connected(&adjacency) {
            if group.len() < 2 {
                continue;
            }
            let members: BTreeSet<String> = group.iter().map(|&at| names[at].clone()).collect();
            let spoken = found.repo.unit_name(unit);
            if let Some(at) = crate::core::record::quiets_cycle(subjects, &spoken, &members) {
                live.insert(at);
                continue;
            }
            let home = &found.repo.units[unit].path;
            let place = if folder.is_empty() {
                if home.as_os_str().is_empty() {
                    ".".to_string()
                } else {
                    home.to_string_lossy().to_string()
                }
            } else {
                home.join(&folder).to_string_lossy().to_string()
            };
            let listed: Vec<&str> = members.iter().map(String::as_str).collect();
            findings.push(Finding::integrity(
                Path::new(&place),
                0,
                "sibling-cycle",
                format!(
                    "the children of {place} form a cycle: {}",
                    listed.join(" <-> ")
                ),
            ));
        }
    }
    (findings, live)
}
