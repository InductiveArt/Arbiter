use std::collections::HashSet;
use std::path::Path;

use crate::core::prose;
use crate::packs::infra::reading::FloatingImage;
use crate::packs::infra::reading::Overreach;
use crate::packs::infra::reading::Reading;

// One manifest read as text, when it is a surface: its comments, then, in a
// Kubernetes document, each image it pulls by tag.
pub fn read(file: &Path, source: &str, surface: bool) -> Option<Reading> {
    if !surface {
        return None;
    }
    let mut reading = Reading {
        file: file.to_path_buf(),
        ..Reading::default()
    };
    // A comment is read in any YAML; everything outside comments is the
    // body a named thing may stand in.
    let mut comments: Vec<(String, usize)> = Vec::new();
    let mut body = String::new();
    for (at, line) in source.lines().enumerate() {
        let split = line.char_indices().find(|(index, part)| {
            *part == '#' && (*index == 0 || line[..*index].ends_with([' ', '\t']))
        });
        match split {
            Some((index, _)) => {
                body.push_str(&line[..index]);
                comments.push((line[index..].to_string(), at + 1));
            }
            None => body.push_str(line),
        }
        body.push('\n');
    }
    let idents = HashSet::new();
    for (token, line) in prose::overreaches(&comments, &idents, &body) {
        reading.overreaches.push(Overreach { token, line });
    }
    // Only a Kubernetes document names images; other YAML passes untouched.
    if !source.contains("apiVersion:") {
        return Some(reading);
    }
    for (at, line) in source.lines().enumerate() {
        let trimmed = line.trim_start().trim_start_matches("- ").trim_start();
        let Some(value) = trimmed.strip_prefix("image:") else {
            continue;
        };
        let image = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        if image.is_empty() || image.contains("@sha256:") {
            continue;
        }
        reading.floating.push(FloatingImage {
            image,
            line: at + 1,
        });
    }
    Some(reading)
}
