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

// One case as seated: where it is implemented and which parent need it serves.
pub struct Case {
    pub name: String,
    pub unit: Option<String>,
    pub serves: Option<String>,
}

#[derive(PartialEq)]
pub enum State {
    Held,
    Planned,
    Manual,
    Unanswered,
}

// One door move of a trial: a request the examiner sends itself.
pub struct Move {
    pub method: String,
    pub route: String,
    pub body: String,
}

// One case's trial: the world the case runs in and the moves the examiner
// performs through its door, with the raw probe reading the store. The
// choreography is arbiter's; the record supplies coordinates and moves only.
pub struct Trial {
    pub case: String,
    // The mandate: the need sentences of its own case this trial proves.
    pub proves: Vec<String>,
    pub sequence: String,
    pub store: String,
    pub volume: String,
    pub path: String,
    pub create: Move,
    pub update: Move,
    pub delete: Move,
    pub read: Move,
    // The exact body the read move must be answered with.
    pub answer: String,
    pub probe: String,
    pub created: String,
    pub updated: String,
    pub seed: String,
}

pub struct Reading {
    pub cases: Vec<Case>,
    pub needs: Vec<Need>,
    pub findings: Vec<Finding>,
    pub claims: usize,
    pub unwired: Vec<(PathBuf, usize, String)>,
    pub trials: Vec<Trial>,
}

fn trial_of(case_name: &str, case: &toml::Value) -> Option<Trial> {
    let trial = case.get("trial")?;
    let word = |field: &str| {
        trial
            .get(field)
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    };
    let move_of = |field: &str| -> Option<Move> {
        let entry = trial.get(field)?;
        Some(Move {
            method: entry.get("method")?.as_str()?.to_string(),
            route: entry.get("route")?.as_str()?.to_string(),
            body: entry
                .get("body")
                .and_then(toml::Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
    };
    Some(Trial {
        case: case_name.to_string(),
        proves: trial
            .get("proves")
            .and_then(toml::Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        sequence: word("sequence").unwrap_or_else(|| "persistence".to_string()),
        store: word("store")?,
        volume: word("volume")?,
        path: word("path")?,
        create: move_of("create")?,
        update: move_of("update")?,
        delete: move_of("delete")?,
        read: move_of("read")?,
        answer: trial
            .get("read")
            .and_then(|entry| entry.get("answer"))
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string(),
        probe: word("probe")?,
        created: word("created")?,
        updated: word("updated")?,
        seed: word("seed")?,
    })
}

// A binding names a claim exactly, or a frame it covers whole: a prefix ends
// at a boundary, so tomorrow's claims under a bound frame wire themselves.
// Runs of spaces fold before comparing: an identity with no frames prints a
// double space a hand will type as one.
fn wired(entry: &str, identity: &str) -> bool {
    let folded = |text: &str| text.split_whitespace().collect::<Vec<&str>>().join(" ");
    let entry = folded(entry);
    let identity = folded(identity);
    identity == entry
        || (identity.starts_with(&entry)
            && (identity[entry.len()..].starts_with(" | ")
                || identity[entry.len()..].starts_with(" / ")))
}

// The first line spelling the needle, so a finding points into the brief; 0
// when the file never spells it whole.
fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .map_or(0, |at| at + 1)
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
                cases: Vec::new(),
                needs: Vec::new(),
                findings: vec![Finding {
                    check: "brief",
                    file: brief_path,
                    line: 0,
                    name: "unparsable-user-record".to_string(),
                    reason: format!("{BRIEF} does not parse"),
                }],
                claims: 0,
                unwired: Vec::new(),
                trials: Vec::new(),
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
    let mut trials = Vec::new();
    let mut seated = Vec::new();
    let mut covered = vec![false; spoken.len()];
    for case in cases {
        let name = case
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or("unnamed")
            .to_string();
        if let Some(trial) = trial_of(&name, case) {
            trials.push(trial);
        }
        seated.push(Case {
            name: name.clone(),
            unit: case
                .get("unit")
                .and_then(toml::Value::as_str)
                .map(str::to_string),
            serves: case
                .get("serves")
                .and_then(toml::Value::as_str)
                .map(str::to_string),
        });
        let listed = case
            .get("need")
            .and_then(toml::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if listed.is_empty() {
            findings.push(Finding {
                check: "brief",
                file: brief_path.clone(),
                line: line_of(&text, &format!("name = \"{name}\"")),
                name: "empty-case".to_string(),
                reason: format!(
                    "the case '{name}' declares no needs; a case with no needs proves nothing"
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
                            line: line_of(&text, binding),
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
                    line: line_of(&text, &format!("need = \"{need}\"")),
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
    for case in &seated {
        if let Some(unit) = &case.unit {
            let built =
                (0..found.repo.units.len()).any(|index| found.repo.unit_name(index) == *unit);
            if !built {
                findings.push(Finding {
                    check: "brief",
                    file: brief_path.clone(),
                    line: line_of(&text, &format!("unit = \"{unit}\"")),
                    name: "misplaced-case".to_string(),
                    reason: format!(
                        "the case '{}' is placed in unit {unit}, which this repository does not build",
                        case.name
                    ),
                });
            }
        }
        if let Some(target) = &case.serves {
            let stated = needs
                .iter()
                .any(|need| need.case != case.name && need.need == *target);
            if !stated {
                findings.push(Finding {
                    check: "brief",
                    file: brief_path.clone(),
                    line: line_of(&text, target),
                    name: "orphan-case".to_string(),
                    reason: format!(
                        "the case '{}' serves '{target}', which no other case states as a need",
                        case.name
                    ),
                });
            }
        }
    }
    for trial in &trials {
        if trial.proves.is_empty() {
            findings.push(Finding {
                check: "brief",
                file: brief_path.clone(),
                line: line_of(&text, "[case.trial]"),
                name: "unmandated-trial".to_string(),
                reason: format!(
                    "the trial of case '{}' proves nothing; a trial serves stated needs or does not sit",
                    trial.case
                ),
            });
        }
        for proof in &trial.proves {
            let stated = needs
                .iter()
                .any(|need| need.case == trial.case && need.need == *proof);
            if !stated {
                findings.push(Finding {
                    check: "brief",
                    file: brief_path.clone(),
                    line: line_of(&text, proof),
                    name: "stale-mandate".to_string(),
                    reason: format!(
                        "the trial of case '{}' proves '{proof}', which the case does not state as a need",
                        trial.case
                    ),
                });
            }
        }
    }
    Some(Reading {
        cases: seated,
        needs,
        findings,
        claims: spoken.len(),
        unwired,
        trials,
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
