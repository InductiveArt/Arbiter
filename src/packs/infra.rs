use std::path::{Path, PathBuf};

use crate::core::pack::{Def, Definition, Edge, Finding, Pack};
use crate::core::units::Repo;

// Deployment manifests as judged text: the pack defines no symbols and runs
// no tools; it holds the written state to the laws text alone can prove.
#[derive(Default)]
pub struct Infra {
    floating: Vec<(PathBuf, usize, String)>,
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
        // Only a Kubernetes document names images; other YAML passes untouched.
        if !surface || !source.contains("apiVersion:") {
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
        self.floating
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
            .collect()
    }

    fn resolve(&self, _mine: usize, _defs: &[Def], _root: &Path, _repo: &Repo) -> Vec<Edge> {
        Vec::new()
    }
}
