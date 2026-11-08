use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::core::pack::Finding;
use crate::core::units::fits;

pub const SUPPRESSIONS: &str = ".arbiter/suppressions.toml";

// One package of one entry: the grain the law honors, the ledger counts and
// staleness judges.
pub struct Subject {
    pub language: String,
    pub package: String,
    pub units: Vec<String>,
    pub line: usize,
}

impl Subject {
    // The identity the reference absorbs: the written decision, never a line
    // number alone, which moves with every edit above it.
    pub fn identity(&self) -> String {
        format!(
            "unused-dependency {} {} {}",
            self.language,
            self.package,
            self.units.join(" ")
        )
    }
}

// The record whole: the subjects a pack may honor, and the findings the file
// earns. The record quiets unused-dependency alone; a form wider than its
// named subjects is a blanket, refused and never recorded.
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
        let rule = entry.get("rule").and_then(toml::Value::as_str);
        if rule != Some("unused-dependency") {
            findings.push(blanket(match rule {
                None => {
                    "an entry names no rule; a suppression names the one rule it quiets".to_string()
                }
                Some(other) => format!(
                    "{other} is fixed, never recorded; the record quiets unused-dependency alone"
                ),
            }));
            continue;
        }
        let Some(language) = entry.get("language").and_then(toml::Value::as_str) else {
            findings.push(blanket(
                "an entry names no language; a suppression covers one manifest's law".to_string(),
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
            subjects.push(Subject {
                language: language.to_string(),
                package,
                units: units.clone(),
                line,
            });
        }
    }
    (subjects, findings)
}

// The subject quieting this package in this unit, so the pack skips the
// finding and the subject is proven alive.
pub fn quiets(subjects: &[Subject], language: &str, unit: &str, package: &str) -> Option<usize> {
    subjects.iter().position(|subject| {
        subject.language == language
            && subject.package == package
            && subject.units.iter().any(|pattern| fits(pattern, unit))
    })
}

// A subject that quieted nothing this run is named: the package is gone, or
// imported after all, and a suppression that suppresses nothing is stale.
pub fn stale(
    subjects: &[Subject],
    language: &str,
    live: &std::collections::HashSet<usize>,
) -> Vec<Finding> {
    subjects
        .iter()
        .enumerate()
        .filter(|(at, subject)| subject.language == language && !live.contains(at))
        .map(|(_, subject)| Finding {
            check: "integrity",
            file: PathBuf::from(SUPPRESSIONS),
            line: subject.line,
            name: "stale-suppression".to_string(),
            reason: format!(
                "the suppression of unused-dependency on {} quiets nothing",
                subject.package
            ),
        })
        .collect()
}
