use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::str;

use serde_json::Value;

use crate::core::digest::Digest;
use crate::domain::ecosystem::Ecosystem;
use crate::domain::file::File;
use crate::domain::marker::Marker;

// Every marker among the listed files, sorted by path, then by ecosystem. A
// marker is opened only when its ecosystem has a rule to read it by.
pub fn read_markers(
    root: &Path,
    files: &[File],
    ecosystems: &[Ecosystem],
    checkpoint: &mut dyn FnMut() -> Result<(), String>,
) -> Result<Vec<Marker>, String> {
    let mut markers = Vec::new();
    for file in files {
        let Some(name) = file.path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        for ecosystem in ecosystems {
            if ecosystem.marker != name {
                continue;
            }
            checkpoint()?;
            let members_only = match &ecosystem.members_only {
                None => false,
                Some(rule) => rule.fits(&document(root, file)?),
            };
            markers.push(Marker {
                path: file.path.clone(),
                ecosystem: ecosystem.name.clone(),
                members_only,
            });
        }
    }
    markers.sort_by(|one, other| {
        one.path
            .cmp(&other.path)
            .then_with(|| one.ecosystem.cmp(&other.ecosystem))
    });
    Ok(markers)
}

// The marker's content as a tree of keys. It must still be the content that
// was listed, or what is told of it would not belong to the list.
fn document(root: &Path, file: &File) -> Result<Value, String> {
    let shown = file.path.display();
    let content = fs::read(root.join(&file.path))
        .map_err(|error| format!("{shown} could not be read: {error}"))?;
    let mut digest = Digest::new();
    digest.eat(&content);
    if digest.hex() != file.digest {
        return Err(format!("{shown} changed since it was listed"));
    }
    match file.path.extension().and_then(OsStr::to_str) {
        Some("toml") => {
            let text = str::from_utf8(&content)
                .map_err(|error| format!("{shown} could not be read as toml: {error}"))?;
            toml::from_str(text).map_err(|error| {
                let told: Vec<&str> = error.message().lines().map(str::trim).collect();
                format!("{shown} could not be read as toml: {}", told.join(", "))
            })
        }
        Some("json") => serde_json::from_slice(&content)
            .map_err(|error| format!("{shown} could not be read as json: {error}")),
        Some("yaml" | "yml") => str::from_utf8(&content)
            .map(top_level_keys)
            .map_err(|error| format!("{shown} could not be read as yaml: {error}")),
        _ => Err(format!(
            "{shown} is neither toml, json nor yaml: no rule can be read from it"
        )),
    }
}

// The keys of a YAML document at no indent, all a members-only rule reads in
// one: a line starting with a key and its colon, never a comment, a list
// item or a line within a block.
fn top_level_keys(text: &str) -> Value {
    let keys = text
        .lines()
        .filter(|line| !line.starts_with([' ', '\t', '#', '-']))
        .filter_map(|line| line.split_once(':'))
        .map(|(key, _)| (key.trim().to_string(), Value::Bool(true)))
        .collect();
    Value::Object(keys)
}
