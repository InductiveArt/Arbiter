use std::path::Path;

use crate::domain::finding::Finding;
use crate::packs::rust::language::MANIFEST;
use crate::packs::rust::manifest::table_at;

// The bar denies every warning, so a lint turned to allow in the manifest lowers it.
pub fn instances(parsed: &toml::Value, home: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    for base in ["lints", "workspace.lints"] {
        for group in ["rust", "clippy"] {
            let path = format!("{base}.{group}");
            let Some(table) = table_at(parsed, &path) else {
                continue;
            };
            for (name, level) in table {
                if allows(level) {
                    findings.push(Finding::dial(
                        home,
                        MANIFEST,
                        "lowered-dial",
                        format!("{path}.{name} is allow; the bar denies every warning"),
                    ));
                }
            }
        }
    }
    findings
}

fn allows(level: &toml::Value) -> bool {
    level.as_str() == Some("allow")
        || level.get("level").and_then(toml::Value::as_str) == Some("allow")
}
