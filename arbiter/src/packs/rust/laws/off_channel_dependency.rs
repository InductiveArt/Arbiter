use std::path::Path;

use crate::domain::finding::Finding;
use crate::packs::rust::language::MANIFEST;
use crate::packs::rust::manifest::table_at;

pub fn instances(parsed: &toml::Value, home: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    // The registry and the repository are the only channels: a git source slips
    // past the lockfile's hashes and past the split-version rule.
    for base in [
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "workspace.dependencies",
    ] {
        let Some(table) = table_at(parsed, base) else {
            continue;
        };
        for (name, spec) in table {
            if spec.get("git").is_some() {
                findings.push(Finding::dial(
                    home,
                    MANIFEST,
                    "off-channel-dependency",
                    format!(
                        "{base}.{name} comes from git; \
                         a dependency comes from the locked registry or this repository"
                    ),
                ));
            } else if spec
                .get("path")
                .and_then(toml::Value::as_str)
                .is_some_and(|path| Path::new(path).is_absolute())
            {
                findings.push(Finding::dial(
                    home,
                    MANIFEST,
                    "off-channel-dependency",
                    format!(
                        "{base}.{name} is an absolute path; \
                         a path dependency stays inside the repository"
                    ),
                ));
            }
        }
    }
    findings
}
