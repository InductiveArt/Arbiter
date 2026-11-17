use std::path::Path;
use std::path::PathBuf;

use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::packs::python::distributions::normalized_name;
use crate::packs::python::language::MANIFEST;

// Each third party's license as its wheel installed it in the unit's own
// environment, by the manifest that pulled it.
pub fn licenses(root: &Path, repo: &Repo) -> Vec<(PathBuf, String, String)> {
    let mut found = Vec::new();
    for manifest in &repo.manifests {
        if manifest.file_name().is_none_or(|name| name != MANIFEST) {
            continue;
        }
        let unit = repo.unit_of(manifest);
        let home = repo.units[unit].path.clone();
        if is_test(&home) {
            continue;
        }
        let folder = manifest.parent().unwrap_or(Path::new(""));
        // The unit's own package sits in its venv too, and is nobody's third party.
        let own = std::fs::read_to_string(root.join(manifest))
            .ok()
            .and_then(|text| text.parse::<toml::Value>().ok())
            .and_then(|parsed| {
                parsed
                    .get("project")?
                    .get("name")
                    .and_then(toml::Value::as_str)
                    .map(normalized_name)
            });
        let lib = root.join(folder).join(".venv").join("lib");
        let Ok(pythons) = std::fs::read_dir(&lib) else {
            continue;
        };
        for python in pythons.flatten() {
            let site = python.path().join("site-packages");
            let Ok(entries) = std::fs::read_dir(&site) else {
                continue;
            };
            for entry in entries.flatten() {
                if !entry.file_name().to_string_lossy().ends_with(".dist-info") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(entry.path().join("METADATA")) else {
                    continue;
                };
                if let Some((name, license)) = metadata_license(&text) {
                    if own.as_deref() == Some(normalized_name(&name).as_str()) {
                        continue;
                    }
                    found.push((manifest.clone(), name, license));
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

// The package and its license from wheel metadata: the SPDX expression where one
// is stated, else the License field, else what the classifier names.
fn metadata_license(text: &str) -> Option<(String, String)> {
    let mut name = None;
    let mut expression = None;
    let mut field = None;
    let mut classifier = None;
    for line in text.lines() {
        // The headers end at the first blank line; the description follows.
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Name: ") {
            name = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("License-Expression: ") {
            expression = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("License: ") {
            field = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("Classifier: License :: OSI Approved :: ") {
            classifier = Some(value.trim().trim_end_matches(" License").to_string());
        } else if let Some(value) = line.strip_prefix("Classifier: License :: ") {
            classifier.get_or_insert(value.trim().to_string());
        }
    }
    let license = expression
        .or_else(|| field.filter(|value| !value.is_empty() && value != "UNKNOWN"))
        .or(classifier)
        .unwrap_or_else(|| "unstated".to_string());
    Some((name?, license))
}

#[cfg(test)]
mod tests {
    use super::metadata_license;

    #[test]
    fn a_license_comes_from_the_expression_the_field_or_the_classifier() {
        let modern = "Name: alpha\nLicense-Expression: MIT OR Apache-2.0\n\nBody";
        let fielded = "Name: beta\nLicense: BSD-3-Clause\n\nBody";
        let classified =
            "Name: gamma\nLicense: UNKNOWN\nClassifier: License :: OSI Approved :: MIT License\n";
        assert_eq!(
            metadata_license(modern),
            Some(("alpha".to_string(), "MIT OR Apache-2.0".to_string()))
        );
        assert_eq!(
            metadata_license(fielded),
            Some(("beta".to_string(), "BSD-3-Clause".to_string()))
        );
        assert_eq!(
            metadata_license(classified),
            Some(("gamma".to_string(), "MIT".to_string()))
        );
    }
}
