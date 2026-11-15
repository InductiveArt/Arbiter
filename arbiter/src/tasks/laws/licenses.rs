use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use crate::core::graph::Graph;
use crate::core::pack::Finding;
use crate::core::rule;
use crate::core::rule::Law;
use crate::core::rule::Rule;

const LICENSES: &str = ".arbiter/licenses.toml";

pub const RULES: &[Rule] = &[rule::record(
    "unallowed-license",
    "third-parties",
    ".arbiter/licenses.toml",
    "a written allowlist holds every third party's license; no list is no policy yet",
)];

pub fn licenses_law() -> Law {
    Law {
        task: "licenses",
        rules: RULES,
        run: judge,
    }
}

// A written allowlist turns the licenses the ecosystems materialized into
// judgments; without the file, no license policy exists to hold anything to.
fn judge(found: &Graph) -> Vec<Finding> {
    let Ok(text) = fs::read_to_string(found.root.join(LICENSES)) else {
        return Vec::new();
    };
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return vec![Finding {
            check: "licenses",
            file: PathBuf::from(LICENSES),
            line: 0,
            name: "unparsable-user-record".to_string(),
            reason: format!("{LICENSES} does not parse"),
        }];
    };
    // No allow key is no policy yet, the seeded file's state; an empty list
    // written out allows nothing.
    let Some(allowed) = parsed
        .get("allow")
        .and_then(toml::Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(toml::Value::as_str)
                .collect::<Vec<&str>>()
        })
    else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();
    for pack in &found.packs {
        for (place, package, license) in pack.licenses(&found.root, &found.repo) {
            if permitted(&license, &allowed) {
                continue;
            }
            if !seen.insert((package.clone(), license.clone())) {
                continue;
            }
            findings.push(Finding {
                check: "licenses",
                file: place,
                line: 0,
                name: "unallowed-license".to_string(),
                reason: format!(
                    "{package} is licensed {license}, which the allowlist does not name"
                ),
            });
        }
    }
    findings
}

// An SPDX expression passes when one OR branch does and every AND side does;
// a common spelling folds to its SPDX name before the match.
fn permitted(license: &str, allowed: &[&str]) -> bool {
    let text = license.trim();
    if text.contains(" OR ") {
        return text.split(" OR ").any(|part| permitted(part, allowed));
    }
    if text.contains(" AND ") {
        return text.split(" AND ").all(|part| permitted(part, allowed));
    }
    let name = folded(text);
    allowed.iter().any(|allow| folded(allow) == name)
}

fn folded(name: &str) -> String {
    let trimmed = name
        .trim()
        .trim_matches(|part: char| part == '(' || part == ')')
        .trim();
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "mit license" | "the mit license" => "mit".to_string(),
        "apache 2.0"
        | "apache license 2.0"
        | "apache license, version 2.0"
        | "apache software license" => "apache-2.0".to_string(),
        "3-clause bsd license" | "bsd 3-clause" | "new bsd license" | "bsd license" => {
            "bsd-3-clause".to_string()
        }
        "2-clause bsd license" | "bsd 2-clause" | "simplified bsd license" => {
            "bsd-2-clause".to_string()
        }
        "isc license" => "isc".to_string(),
        _ => lower,
    }
}

#[cfg(test)]
mod tests {
    use super::permitted;

    #[test]
    fn one_allowed_branch_of_an_or_passes() {
        let allowed = ["Apache-2.0", "MIT"];
        assert!(permitted("Apache-2.0 OR BSD-2-Clause", &allowed));
        assert!(permitted("(MIT OR GPL-3.0-only)", &allowed));
        assert!(!permitted("GPL-3.0-only OR AGPL-3.0-only", &allowed));
        assert!(!permitted("Apache-2.0 AND GPL-3.0-only", &allowed));
        assert!(permitted("Apache-2.0 AND MIT", &allowed));
    }

    #[test]
    fn a_common_spelling_folds_to_its_spdx_name() {
        let allowed = ["BSD-3-Clause", "Apache-2.0"];
        assert!(permitted("3-Clause BSD License", &allowed));
        assert!(permitted("Apache Software License", &allowed));
        assert!(permitted("apache-2.0", &allowed));
        assert!(!permitted("WTFPL", &allowed));
    }
}
