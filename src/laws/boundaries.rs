use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::core::graph::Graph;
use crate::core::measures::listed;
use crate::core::measures::sanctioned;
use crate::core::measures::shown;
use crate::core::pack::Finding;
use crate::core::rule;
use crate::core::rule::Law;
use crate::core::rule::Rule;
use crate::core::units::DECLARATIONS;

pub const RULES: &[Rule] = &[
    rule::record(
        "uninvited-unit-use",
        "units",
        ".arbiter/units.toml",
        "a unit is used only by the units its declared visibility grants",
    ),
    rule::record(
        "stale-unit-grant",
        "units",
        ".arbiter/units.toml",
        "a visibility grant names a unit that uses it; one never used is deleted",
    ),
    rule::record(
        "undeclared-unit-use",
        "units",
        ".arbiter/units.toml",
        "a unit declaring deps names every unit it uses",
    ),
    rule::record(
        "stale-unit-declaration",
        "units",
        ".arbiter/units.toml",
        "a declared unit dep is used; one never used is deleted",
    ),
];

pub fn boundaries_law() -> Law {
    Law {
        task: "boundaries",
        rules: RULES,
        run: judge,
    }
}

// The declared unit graph, held against the resolved one. A unit declaring nothing is
// unchecked: the file is adopted one unit at a time, the way a baseline is.
fn judge(found: &Graph) -> Vec<Finding> {
    let Ok(text) = fs::read_to_string(found.root.join(DECLARATIONS)) else {
        return Vec::new();
    };
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return vec![Finding::integrity(
            Path::new(DECLARATIONS),
            0,
            "unparsable-user-record",
            format!("{DECLARATIONS} does not parse; the declared boundaries cannot be read"),
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
    for (user, owned, file, line) in &witness {
        let (invited, declared) = sanctioned(units, user, owned);
        if !invited {
            let patterns = listed(units, owned, "visibility").unwrap_or_default();
            findings.push(Finding::integrity(
                file,
                *line,
                "uninvited-unit-use",
                format!(
                    "{user} uses {owned}, which only {} may",
                    patterns.join(", ")
                ),
            ));
        }
        if !declared {
            findings.push(Finding::integrity(
                file,
                *line,
                "undeclared-unit-use",
                format!("{user} uses {owned} and does not declare it"),
            ));
        }
    }
    for (name, _) in units {
        if let Some(patterns) = listed(units, name, "visibility") {
            // A starred grant spans units that may not exist yet; only a named one can be stale.
            for pattern in patterns {
                if pattern.contains('*') {
                    continue;
                }
                let used = between
                    .iter()
                    .any(|(user, owned)| owned == name && *user == pattern);
                if !used {
                    findings.push(Finding::integrity(
                        Path::new(DECLARATIONS),
                        0,
                        "stale-unit-grant",
                        format!("{name} grants {pattern}, which never uses it"),
                    ));
                }
            }
        }
        if let Some(declared) = listed(units, name, "deps") {
            for dependency in declared {
                let used = between.contains(&(name.clone(), dependency.clone()));
                if !used {
                    findings.push(Finding::integrity(
                        Path::new(DECLARATIONS),
                        0,
                        "stale-unit-declaration",
                        format!("{name} declares {dependency} and never uses it"),
                    ));
                }
            }
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use crate::core::units::fits;

    #[test]
    fn a_star_covers_the_folder_and_everything_below() {
        assert!(fits("apps/*", "apps/api"));
        assert!(fits("apps/*", "apps/web/admin"));
        assert!(!fits("apps/*", "apps-extra"));
        assert!(fits("libs/core", "libs/core"));
        assert!(!fits("libs/core", "libs/core_py"));
    }
}
