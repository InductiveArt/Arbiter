use std::path::Path;

use crate::core::pack::dial;
use crate::domain::finding::Finding;
use crate::packs::python::language::BAR;
use crate::packs::python::language::MANIFEST;
use crate::packs::python::laws::lookup;

const CHECKERS: [&str; 2] = ["basedpyright", "pyright"];

const MODES: [&str; 3] = ["strict", "recommended", "all"];

// Each setting a manifest lowers the bar with: one the bar refuses outright,
// and a type checker's mode, rule or exclude set below it.
pub fn instances(parsed: &toml::Value, home: &Path) -> Vec<Finding> {
    let mut findings = refused(parsed, home);
    findings.extend(checker(parsed, home));
    findings
}

fn refused(parsed: &toml::Value, home: &Path) -> Vec<Finding> {
    let bar = BAR.parse::<toml::Value>().expect("a pack's bar parses");
    let Some(refused) = bar.get("refused").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for (path, reason) in refused {
        if lookup(parsed, path).is_some() {
            findings.push(dial(
                home,
                MANIFEST,
                "lowered-dial",
                format!("{path} is set; {}", reason.as_str().unwrap_or("")),
            ));
        }
    }
    findings
}

fn checker(parsed: &toml::Value, home: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    for checker in CHECKERS {
        let table = parsed
            .get("tool")
            .and_then(|tool| tool.get(checker))
            .and_then(toml::Value::as_table);
        let Some(table) = table else {
            continue;
        };
        for (name, value) in table {
            if name == "typeCheckingMode" {
                let mode = value.as_str().unwrap_or("");
                if !MODES.contains(&mode) {
                    findings.push(dial(
                        home,
                        MANIFEST,
                        "lowered-dial",
                        format!(
                            "tool.{checker}.typeCheckingMode is {mode}; the bar starts at strict"
                        ),
                    ));
                }
            } else if name.starts_with("report") && !is_error(value) {
                findings.push(dial(
                    home,
                    MANIFEST,
                    "lowered-dial",
                    format!(
                        "tool.{checker}.{name} is lowered; a rule reports an error or stays unset"
                    ),
                ));
            } else if name == "exclude" {
                for entry in value.as_array().cloned().unwrap_or_default() {
                    let glob = entry.as_str().unwrap_or("");
                    if !glob.contains("_pb2") {
                        findings.push(dial(
                            home,
                            MANIFEST,
                            "lowered-dial",
                            format!("tool.{checker}.exclude hides {glob}; only generated code leaves the checker's sight"),
                        ));
                    }
                }
            }
        }
    }
    findings
}

fn is_error(value: &toml::Value) -> bool {
    value.as_str() == Some("error") || value.as_bool() == Some(true)
}
