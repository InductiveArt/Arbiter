use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

// One thing a check found: the check that found it, the rule it breaks as that
// check names it, the file or the unit it is in by path from the listed
// folder, its line when it has one, and why it is a finding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub check: String,
    pub rule: Option<String>,
    pub file: PathBuf,
    pub line: Option<u32>,
    pub reason: String,
}

impl Finding {
    // A dial unset or lowered, placed at the unit's config file.
    pub fn dial(home: &Path, config: &str, rule: &str, reason: String) -> Self {
        Finding {
            check: "ecosystem-dials".to_string(),
            file: home.join(config),
            line: None,
            rule: Some(rule.to_string()),
            reason,
        }
    }

    // The finding as every JSON line and snapshot writes it; a finding with no
    // line is written at line 0, which readers count as none.
    pub fn written(&self) -> serde_json::Value {
        serde_json::json!({
            "check": self.check,
            "file": self.file.to_string_lossy(),
            "line": self.line.unwrap_or(0),
            "rule": self.rule,
            "reason": self.reason,
        })
    }

    // A finding of the integrity check: a law's rule broken at a place.
    pub fn integrity(file: &Path, line: usize, rule: &str, reason: String) -> Self {
        Finding {
            check: "integrity".to_string(),
            file: file.to_path_buf(),
            line: line_at(line),
            rule: Some(rule.to_string()),
            reason,
        }
    }
}

// A line as readers count it, from one: zero is no line, as a manifest read
// whole or a cycle between units has none.
pub fn line_at(row: usize) -> Option<u32> {
    u32::try_from(row).ok().filter(|line| *line > 0)
}
