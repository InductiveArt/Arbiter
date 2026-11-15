use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::core::pack::Finding;
use crate::core::units::fits;

pub const SUPPRESSIONS: &str = ".arbiter/suppressions.toml";

// One signed exception: a package one rule may not name, or one exact cycle
// the sibling law may not break. The grain is one subject per entry row, the
// shape the ledger counts and staleness judges.
pub enum Subject {
    Dependency {
        language: String,
        package: String,
        units: Vec<String>,
        line: usize,
    },
    Cycle {
        unit: String,
        members: BTreeSet<String>,
        line: usize,
    },
}

impl Subject {
    // The identity the reference absorbs: the written decision, never a line
    // number alone, which moves with every edit above it.
    pub fn identity(&self) -> String {
        match self {
            Subject::Dependency {
                language,
                package,
                units,
                ..
            } => format!("unused-dependency {language} {package} {}", units.join(" ")),
            Subject::Cycle { unit, members, .. } => {
                let listed: Vec<&str> = members.iter().map(String::as_str).collect();
                format!("sibling-cycle {unit} {}", listed.join(" "))
            }
        }
    }

    pub fn line(&self) -> usize {
        match self {
            Subject::Dependency { line, .. } | Subject::Cycle { line, .. } => *line,
        }
    }
}

// The record whole: the subjects the laws may honor, and the findings the
// file earns. The record quiets unused-dependency and sibling-cycle alone; a
// form wider than its named subjects is a blanket, refused and never
// recorded.
pub fn read(root: &Path) -> (Vec<Subject>, Vec<Finding>) {
    let place = PathBuf::from(SUPPRESSIONS);
    let Ok(text) = fs::read_to_string(root.join(SUPPRESSIONS)) else {
        return (Vec::new(), Vec::new());
    };
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return (
            Vec::new(),
            vec![Finding {
                check: "ledger",
                file: place,
                line: 0,
                name: "unparsable-user-record".to_string(),
                reason: format!("{SUPPRESSIONS} does not parse; the record quiets nothing"),
            }],
        );
    };
    let entries = parsed
        .get("suppression")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let line_of = |needle: &str| {
        text.lines()
            .position(|line| line.contains(needle))
            .map_or(0, |at| at + 1)
    };
    let mut subjects = Vec::new();
    let mut findings = Vec::new();
    let blanket = |reason: String| Finding {
        check: "ledger",
        file: place.clone(),
        line: 0,
        name: "blanket-suppression".to_string(),
        reason,
    };
    for entry in &entries {
        let listed = |name: &str| -> Vec<String> {
            entry
                .get(name)
                .and_then(toml::Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(toml::Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };
        let word = |name: &str| entry.get(name).and_then(toml::Value::as_str);
        match word("rule") {
            Some("unused-dependency") => {
                let Some(language) = word("language") else {
                    findings.push(blanket(
                        "an entry names no language; a suppression covers one manifest's law"
                            .to_string(),
                    ));
                    continue;
                };
                let packages = listed("packages");
                if packages.is_empty() || packages.iter().any(|package| package == "*") {
                    findings.push(blanket(format!(
                        "an entry for {language} covers every package; a blanket form is refused"
                    )));
                    continue;
                }
                let units = listed("units");
                if units.is_empty() {
                    findings.push(blanket(format!(
                        "an entry for {language} names no units; a suppression covers a stated scope"
                    )));
                    continue;
                }
                for package in packages {
                    let line = line_of(&format!("\"{package}\""));
                    subjects.push(Subject::Dependency {
                        language: language.to_string(),
                        package,
                        units: units.clone(),
                        line,
                    });
                }
            }
            Some("sibling-cycle") => {
                let Some(unit) = word("unit") else {
                    findings.push(blanket(
                        "a cycle entry names no unit; a suppression covers a stated scope"
                            .to_string(),
                    ));
                    continue;
                };
                let members: BTreeSet<String> = listed("members").into_iter().collect();
                if members.len() < 2 || members.iter().any(|member| member.contains('*')) {
                    findings.push(blanket(format!(
                        "a cycle entry for {unit} names no exact members; a cycle is its \
                         whole member set, never a pattern"
                    )));
                    continue;
                }
                let anchor = members.iter().next().cloned().unwrap_or_default();
                subjects.push(Subject::Cycle {
                    unit: unit.to_string(),
                    members,
                    line: line_of(&format!("\"{anchor}\"")),
                });
            }
            None => findings.push(blanket(
                "an entry names no rule; a suppression names the one rule it quiets".to_string(),
            )),
            Some(other) => findings.push(blanket(format!(
                "{other} is fixed, never recorded; the record quiets unused-dependency \
                 and sibling-cycle alone"
            ))),
        }
    }
    (subjects, findings)
}

// The subject quieting this package in this unit, so the pack skips the
// finding and the subject is proven alive.
pub fn quiets(subjects: &[Subject], language: &str, unit: &str, package: &str) -> Option<usize> {
    subjects.iter().position(|subject| match subject {
        Subject::Dependency {
            language: spoken,
            package: named,
            units,
            ..
        } => {
            spoken == language
                && named == package
                && units.iter().any(|pattern| fits(pattern, unit))
        }
        Subject::Cycle { .. } => false,
    })
}

// The subject quieting one exact sibling cycle: the whole member set, so a
// third member joining the loop fires fresh.
pub fn quiets_cycle(subjects: &[Subject], unit: &str, members: &BTreeSet<String>) -> Option<usize> {
    subjects.iter().position(|subject| match subject {
        Subject::Cycle {
            unit: spoken,
            members: named,
            ..
        } => spoken == unit && named == members,
        Subject::Dependency { .. } => false,
    })
}

// A dependency subject that quieted nothing this run is named: the package is
// gone, or imported after all, and a suppression that suppresses nothing is
// stale.
pub fn stale(
    subjects: &[Subject],
    language: &str,
    live: &std::collections::HashSet<usize>,
) -> Vec<Finding> {
    subjects
        .iter()
        .enumerate()
        .filter_map(|(at, subject)| match subject {
            Subject::Dependency {
                language: spoken,
                package,
                line,
                ..
            } if spoken == language && !live.contains(&at) => Some(Finding {
                check: "integrity",
                file: PathBuf::from(SUPPRESSIONS),
                line: *line,
                name: "stale-suppression".to_string(),
                reason: format!("the suppression of unused-dependency on {package} quiets nothing"),
            }),
            _ => None,
        })
        .collect()
}

// A cycle subject whose loop no longer exists is named the same way.
pub fn stale_cycles(subjects: &[Subject], live: &std::collections::HashSet<usize>) -> Vec<Finding> {
    subjects
        .iter()
        .enumerate()
        .filter_map(|(at, subject)| match subject {
            Subject::Cycle { unit, line, .. } if !live.contains(&at) => Some(Finding {
                check: "integrity",
                file: PathBuf::from(SUPPRESSIONS),
                line: *line,
                name: "stale-suppression".to_string(),
                reason: format!("the suppression of sibling-cycle in {unit} quiets nothing"),
            }),
            _ => None,
        })
        .collect()
}
