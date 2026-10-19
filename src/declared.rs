use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::graph::Graph;
use crate::pack::Finding;

const DECLARATIONS: &str = ".arbiter/units.toml";

// The declared unit graph, held against the resolved one. A unit declaring nothing is
// unchecked: the file is adopted one unit at a time, the way a baseline is.
pub fn boundaries(found: &Graph) -> Vec<Finding> {
    let Ok(text) = fs::read_to_string(found.root.join(DECLARATIONS)) else {
        return Vec::new();
    };
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return vec![Finding::integrity(
            Path::new(DECLARATIONS),
            0,
            "boundaries",
            format!("{DECLARATIONS} does not parse"),
        )];
    };
    let Some(units) = parsed.get("units").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    // Each resolved edge between two different units, as (user, owner, first user file).
    let mut between: BTreeSet<(String, String)> = BTreeSet::new();
    let mut witness: Vec<(String, String, PathBuf, usize)> = Vec::new();
    for edge in &found.edges {
        let owner = found.defs[edge.def].unit;
        if owner == edge.unit {
            continue;
        }
        let user = shown(found, edge.unit);
        let owned = shown(found, owner);
        if between.insert((user.clone(), owned.clone())) {
            witness.push((user, owned, edge.file.clone(), edge.line));
        }
    }
    for (name, declaration) in units {
        if let Some(allowed) = declaration
            .get("visibility")
            .and_then(toml::Value::as_array)
        {
            let patterns: Vec<&str> = allowed.iter().filter_map(toml::Value::as_str).collect();
            for (user, owned, file, line) in &witness {
                if owned == name && !patterns.iter().any(|pattern| fits(pattern, user)) {
                    findings.push(Finding::integrity(
                        file,
                        *line,
                        "visibility",
                        format!(
                            "{user} uses {owned}, which only {} may",
                            patterns.join(", ")
                        ),
                    ));
                }
            }
        }
        if let Some(deps) = declaration.get("deps").and_then(toml::Value::as_array) {
            let declared: Vec<&str> = deps.iter().filter_map(toml::Value::as_str).collect();
            for (user, owned, file, line) in &witness {
                if user == name && !declared.contains(&owned.as_str()) {
                    findings.push(Finding::integrity(
                        file,
                        *line,
                        "deps",
                        format!("{user} uses {owned} and does not declare it"),
                    ));
                }
            }
            for dependency in &declared {
                let used = between.contains(&(name.clone(), (*dependency).to_string()));
                if !used {
                    findings.push(Finding::integrity(
                        Path::new(DECLARATIONS),
                        0,
                        "deps",
                        format!("{name} declares {dependency} and never uses it"),
                    ));
                }
            }
        }
    }
    findings
}

fn shown(found: &Graph, unit: usize) -> String {
    let path = &found.repo.units[unit].path;
    if path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        path.to_string_lossy().to_string()
    }
}

// An exact unit path, or a folder and star covering every unit below it.
fn fits(pattern: &str, unit: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    match pattern.strip_suffix("/*") {
        Some(base) => unit == base || unit.starts_with(&format!("{base}/")),
        None => unit == pattern,
    }
}

#[cfg(test)]
mod tests {
    use super::fits;

    #[test]
    fn a_star_covers_the_folder_and_everything_below() {
        assert!(fits("apps/*", "apps/api"));
        assert!(fits("apps/*", "apps/web/admin"));
        assert!(!fits("apps/*", "apps-extra"));
        assert!(fits("libs/core", "libs/core"));
        assert!(!fits("libs/core", "libs/core_py"));
    }
}
