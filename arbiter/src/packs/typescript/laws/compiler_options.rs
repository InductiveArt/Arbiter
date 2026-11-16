use std::path::Path;

use crate::core::pack::dial;
use crate::domain::finding::Finding;
use crate::packs::typescript::language::BAR;

// Each compiler option the bar demands and the resolved options leave unset
// or set lower.
pub fn instances(options: &serde_json::Value, home: &Path) -> Vec<Finding> {
    let bar = BAR.parse::<toml::Value>().expect("a pack's bar parses");
    let Some(required) = bar.get("compilerOptions").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for (name, needed) in required {
        let actual = options.get(name);
        if actual.is_some_and(|value| meets(value, needed)) {
            continue;
        }
        let (kind, found) = match actual {
            None => ("unset-dial", "unset".to_string()),
            Some(value) => ("lowered-dial", value.to_string()),
        };
        findings.push(dial(
            home,
            "tsconfig.json",
            kind,
            format!("compilerOptions.{name} is {found}; the bar says {needed}"),
        ));
    }
    findings
}

fn meets(actual: &serde_json::Value, needed: &toml::Value) -> bool {
    match (actual, needed) {
        (serde_json::Value::Bool(found), toml::Value::Boolean(asked)) => found == asked,
        (serde_json::Value::String(found), toml::Value::String(asked)) => found == asked,
        _ => false,
    }
}
