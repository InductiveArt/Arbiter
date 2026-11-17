use std::path::Path;

use crate::domain::finding::Finding;
use crate::packs::typescript::language::MANIFEST;

const DEPENDENCY_SECTIONS: [&str; 4] = [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
];

const OUTSIDE_CHANNELS: [&str; 7] = [
    "git+", "git:", "github:", "http:", "https:", "file:", "link:",
];

// The registry and the workspace are the only channels: a git, URL or file
// source slips past the lockfile's hashes and past the split-version rule.
pub fn instances(parsed: &serde_json::Value, home: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    for section in DEPENDENCY_SECTIONS {
        let Some(table) = parsed.get(section).and_then(serde_json::Value::as_object) else {
            continue;
        };
        for (name, version) in table {
            let spec = version.as_str().unwrap_or("");
            if OUTSIDE_CHANNELS
                .iter()
                .any(|channel| spec.starts_with(channel))
            {
                findings.push(Finding::dial(
                    home,
                    MANIFEST,
                    "off-channel-dependency",
                    format!(
                        "{section}.{name} is \"{spec}\"; \
                         a dependency comes from the locked registry or the workspace"
                    ),
                ));
            }
        }
    }
    findings
}
