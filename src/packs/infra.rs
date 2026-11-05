use std::path::Path;
use std::path::PathBuf;

use crate::core::pack::Def;
use crate::core::pack::Definition;
use crate::core::pack::Edge;
use crate::core::pack::Finding;
use crate::core::pack::Pack;
use crate::core::prose;
use crate::core::units::Repo;

// Deployment manifests as judged text: the pack defines no symbols and runs
// no tools; it holds the written state to the laws text alone can prove.
#[derive(Default)]
pub struct Infra {
    floating: Vec<(PathBuf, usize, String)>,
    overreaches: Vec<(PathBuf, String, usize)>,
}

impl Infra {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Infra {
    fn rules(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("pinned-image", "held"),
            ("no-comment-overreach", "held"),
            (
                "no-hidden-names",
                "never: a manifest declares state; it names no symbols",
            ),
            (
                "undeclared-dependency",
                "never: a manifest's third parties are images, judged by digest",
            ),
            (
                "unused-dependency",
                "never: a manifest's third parties are images, judged by digest",
            ),
            (
                "suppressions",
                "never: a manifest carries no directive that silences a judge",
            ),
            (
                "resolve",
                "never: a manifest declares state; it imports nothing",
            ),
            (
                "dynamic-import",
                "never: a manifest declares state; it imports nothing",
            ),
            (
                "no-reexport",
                "never: a manifest declares state; it imports nothing",
            ),
            (
                "self-entry",
                "never: a manifest declares state; it imports nothing",
            ),
            (
                "interior-import",
                "never: a manifest declares state; it imports nothing",
            ),
            (
                "one-import-per-line",
                "never: a manifest declares state; it imports nothing",
            ),
            (
                "coverage",
                "never: a manifest declares state; nothing of it executes",
            ),
            (
                "tautology",
                "never: a manifest declares state; it asserts nothing",
            ),
            (
                "drift",
                "never: the proto pack judges any unit's buf.gen.yaml, whatever its language",
            ),
        ]
    }

    fn label(&self) -> &'static str {
        "infra"
    }

    fn handles(&self, name: &str) -> bool {
        name.ends_with(".yaml") || name.ends_with(".yml")
    }

    fn roots(&self, _root: &Path, _repo: &Repo) -> Vec<PathBuf> {
        Vec::new()
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        if !surface {
            return Vec::new();
        }
        // A comment is judged in any YAML; everything outside comments is the
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
        let idents = std::collections::HashSet::new();
        for (token, line) in prose::overreaches(&comments, &idents, &body) {
            self.overreaches.push((file.to_path_buf(), token, line));
        }
        // Only a Kubernetes document names images; other YAML passes untouched.
        if !source.contains("apiVersion:") {
            return Vec::new();
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
            self.floating.push((file.to_path_buf(), at + 1, image));
        }
        Vec::new()
    }

    fn integrity(&self, _root: &Path, _repo: &Repo) -> Vec<Finding> {
        let mut findings: Vec<Finding> = self
            .floating
            .iter()
            .map(|(file, line, image)| Finding {
                check: "integrity",
                file: file.clone(),
                line: *line,
                name: "pinned-image".to_string(),
                reason: format!(
                    "{image} is pulled by tag; an image is named by digest, never by a tag alone"
                ),
            })
            .collect();
        findings.extend(self.overreaches.iter().map(|(file, token, line)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "no-comment-overreach".to_string(),
            reason: format!(
                "a comment names {token}, which nothing in the file defines, uses or quotes"
            ),
        }));
        findings
    }

    fn resolve(&self, _mine: usize, _defs: &[Def], _root: &Path, _repo: &Repo) -> Vec<Edge> {
        Vec::new()
    }
}
