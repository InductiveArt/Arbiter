use std::fs;
use std::path::PathBuf;

use crate::core::graph::Graph;
use crate::core::pack::Finding;
use crate::judge::claims;

const BRIEF: &str = ".arbiter/brief.toml";

// One need of one case, with the state the tree gives it: held by wired
// claims, planned before the code exists, manual where the evidence lives
// outside the repository, or unanswered.
pub struct Need {
    pub case: String,
    pub need: String,
    pub state: State,
    pub entries: Vec<String>,
}

#[derive(PartialEq)]
pub enum State {
    Held,
    Planned,
    Manual,
    Unanswered,
}

pub struct Reading {
    pub cases: usize,
    pub needs: Vec<Need>,
    pub findings: Vec<Finding>,
    pub claims: usize,
    pub unwired: Vec<(PathBuf, usize, String)>,
}

// A binding names a claim exactly, or a frame it covers whole: a prefix ends
// at a boundary, so tomorrow's claims under a bound frame wire themselves.
fn wired(entry: &str, identity: &str) -> bool {
    identity == entry
        || (identity.starts_with(entry)
            && (identity[entry.len()..].starts_with(" | ")
                || identity[entry.len()..].starts_with(" / ")))
}

// The brief is written once it holds a case; a seeded file of comments is not
// yet a brief, the way an absent allowlist is not yet a license policy.
pub fn read(found: &Graph) -> Option<Reading> {
    let text = fs::read_to_string(found.root.join(BRIEF)).ok()?;
    let brief_path = PathBuf::from(BRIEF);
    let parsed = match text.parse::<toml::Value>() {
        Ok(parsed) => parsed,
        Err(_) => {
            return Some(Reading {
                cases: 0,
                needs: Vec::new(),
                findings: vec![Finding {
                    check: "brief",
                    file: brief_path,
                    line: 0,
                    name: "brief".to_string(),
                    reason: format!("{BRIEF} does not parse"),
                }],
                claims: 0,
                unwired: Vec::new(),
            });
        }
    };
    let cases = parsed.get("case").and_then(toml::Value::as_array)?;
    if cases.is_empty() {
        return None;
    }

    let (claimed, _) = claims::heard(found);
    let mut spoken: Vec<(String, PathBuf, usize, String)> = Vec::new();
    for (unit, list) in &claimed {
        for one in list {
            spoken.push((
                claims::identity(unit, &one.claim),
                one.file.clone(),
                one.claim.line,
                one.claim.sentence.clone(),
            ));
        }
    }

    let mut needs = Vec::new();
    let mut findings = Vec::new();
    let mut covered = vec![false; spoken.len()];
    for case in cases {
        let name = case
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or("unnamed")
            .to_string();
        let listed = case
            .get("need")
            .and_then(toml::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if listed.is_empty() {
            findings.push(Finding {
                check: "brief",
                file: brief_path.clone(),
                line: 0,
                name: "empty-case".to_string(),
                reason: format!(
                    "the case '{name}' declares no needs; a parcel with no spec proves nothing"
                ),
            });
            continue;
        }
        for entry in listed {
            let need = entry
                .get("need")
                .and_then(toml::Value::as_str)
                .unwrap_or("unworded")
                .to_string();
            let bindings: Vec<String> = entry
                .get("claims")
                .and_then(toml::Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(toml::Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let planned = entry
                .get("planned")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            let manual = entry.get("manual").and_then(toml::Value::as_str).is_some();
            let state = if !bindings.is_empty() {
                let mut answered = false;
                for binding in &bindings {
                    let mut matched = false;
                    for (at, (identity, _, _, _)) in spoken.iter().enumerate() {
                        if wired(binding, identity) {
                            covered[at] = true;
                            matched = true;
                        }
                    }
                    if matched {
                        answered = true;
                    } else {
                        findings.push(Finding {
                            check: "brief",
                            file: brief_path.clone(),
                            line: 0,
                            name: "stale-binding".to_string(),
                            reason: format!(
                                "the case '{name}' binds '{binding}', which the tree does not speak"
                            ),
                        });
                    }
                }
                if answered {
                    State::Held
                } else {
                    State::Unanswered
                }
            } else if manual {
                State::Manual
            } else if planned {
                State::Planned
            } else {
                findings.push(Finding {
                    check: "brief",
                    file: brief_path.clone(),
                    line: 0,
                    name: "unanswered-need".to_string(),
                    reason: format!(
                        "the need '{need}' of case '{name}' binds no claims and is neither planned nor manual"
                    ),
                });
                State::Unanswered
            };
            needs.push(Need {
                case: name.clone(),
                need,
                state,
                entries: bindings,
            });
        }
    }
    let mut unwired = Vec::new();
    for (at, (identity, file, line, _)) in spoken.iter().enumerate() {
        if !covered[at] {
            findings.push(Finding {
                check: "brief",
                file: file.clone(),
                line: *line,
                name: "unwired-claim".to_string(),
                reason: format!("the claim '{identity}' answers no need in the brief"),
            });
            unwired.push((file.clone(), *line, identity.clone()));
        }
    }
    Some(Reading {
        cases: cases.len(),
        needs,
        findings,
        claims: spoken.len(),
        unwired,
    })
}

// The files whose claims answer a case that still holds a planned need: the
// case's consumer is commissioned, so its tests are building, not orphaned.
pub fn commissioned(found: &Graph) -> std::collections::HashSet<PathBuf> {
    let mut files = std::collections::HashSet::new();
    let Some(reading) = read(found) else {
        return files;
    };
    let (claimed, _) = claims::heard(found);
    let mut spoken: Vec<(String, PathBuf)> = Vec::new();
    for (unit, list) in &claimed {
        for one in list {
            spoken.push((claims::identity(unit, &one.claim), one.file.clone()));
        }
    }
    let planned: std::collections::HashSet<&str> = reading
        .needs
        .iter()
        .filter(|need| need.state == State::Planned)
        .map(|need| need.case.as_str())
        .collect();
    for need in &reading.needs {
        if !planned.contains(need.case.as_str()) {
            continue;
        }
        for entry in &need.entries {
            for (identity, file) in &spoken {
                if wired(entry, identity) {
                    files.insert(file.clone());
                }
            }
        }
    }
    files
}

pub fn findings(found: &Graph) -> Vec<Finding> {
    read(found)
        .map(|reading| reading.findings)
        .unwrap_or_default()
}
