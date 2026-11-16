use std::path::Path;

use crate::core::pack::dial;
use crate::domain::finding::Finding;
use crate::packs::python::language::MANIFEST;
use crate::packs::python::laws::lookup;

// The registry and the repository are the only channels: a git or URL source
// slips past the lockfile's hashes and past the split-version rule.
pub fn instances(parsed: &toml::Value, home: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut lists: Vec<(String, Vec<toml::Value>)> = Vec::new();
    if let Some(list) = lookup(parsed, "project.dependencies").and_then(toml::Value::as_array) {
        lists.push(("project.dependencies".to_string(), list.clone()));
    }
    for path in ["project.optional-dependencies", "dependency-groups"] {
        let Some(groups) = lookup(parsed, path).and_then(toml::Value::as_table) else {
            continue;
        };
        for (group, list) in groups {
            let entries = list.as_array().cloned().unwrap_or_default();
            lists.push((format!("{path}.{group}"), entries));
        }
    }
    for (place, entries) in lists {
        for entry in entries {
            let requirement = entry.as_str().unwrap_or("");
            if requirement.contains("://") || requirement.contains("git+") {
                let name = requirement.split_whitespace().next().unwrap_or(requirement);
                findings.push(dial(
                    home,
                    MANIFEST,
                    "off-channel-dependency",
                    format!(
                        "{place} pulls {name} past the registry; \
                         a dependency comes from the locked registry or this repository"
                    ),
                ));
            }
        }
    }
    let Some(sources) = lookup(parsed, "tool.uv.sources").and_then(toml::Value::as_table) else {
        return findings;
    };
    for (name, source) in sources {
        if source.get("git").is_some() || source.get("url").is_some() {
            findings.push(dial(
                home,
                MANIFEST,
                "off-channel-dependency",
                format!(
                    "tool.uv.sources.{name} points past the registry; \
                     a dependency comes from the locked registry or this repository"
                ),
            ));
        } else if source
            .get("path")
            .and_then(toml::Value::as_str)
            .is_some_and(|path| Path::new(path).is_absolute())
        {
            findings.push(dial(
                home,
                MANIFEST,
                "off-channel-dependency",
                format!(
                    "tool.uv.sources.{name} is an absolute path; \
                     a path source stays inside the repository"
                ),
            ));
        }
    }
    findings
}
