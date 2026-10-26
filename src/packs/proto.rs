use std::path::{Path, PathBuf};
use std::time::Instant;

use tree_sitter::{Language, Tree};

use crate::core::pack::{Def, Definition, Drift, Edge, Pack, Suppression};
use crate::core::trees;
use crate::core::units::Repo;

const BUF: [&str; 3] = ["npx", "--yes", "@bufbuild/buf@1.74.0"];

// What protobuf code generators name their output, in every language a pack reads.
const GENERATED: [&str; 5] = ["_pb2.py", "_pb2.pyi", "_pb2_grpc.py", "_pb.ts", ".pb.ts"];

const DEFINITIONS: &str = include_str!("../../packs/proto/definitions.scm");
const USES: &str = include_str!("../../packs/proto/uses.scm");
const MENTIONS: &str = include_str!("../../packs/proto/mentions.scm");

#[derive(Default)]
pub struct Proto {
    typerefs: Vec<(PathBuf, String, usize)>,
    imports: Vec<(PathBuf, String)>,
}

impl Proto {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Proto {
    fn rules(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("resolve", "held"),
            (
                "dynamic-import",
                "never: a contract names its imports in the file",
            ),
            ("no-reexport", "never: proto has no re-export form"),
            ("self-entry", "never: a contract declares no routes"),
            ("interior-import", "never: a contract declares no routes"),
            ("no-hidden-names", "never: proto has no alias form"),
            (
                "one-import-per-line",
                "never: an import names one file by syntax",
            ),
            ("no-comment-overreach", "held"),
            ("tools", "held"),
            (
                "generated",
                "never: the contract is the source others generate from",
            ),
            ("drift", "held"),
        ]
    }

    fn label(&self) -> &'static str {
        "proto"
    }

    fn handles(&self, name: &str) -> bool {
        name.ends_with(".proto")
    }

    fn axioms(&self) -> &'static [&'static str] {
        &["npx"]
    }

    fn manifest(&self) -> Option<&'static str> {
        Some("buf.yaml")
    }

    fn adapter(&self) -> Option<&'static str> {
        Some(include_str!("../../packs/proto/tools.toml"))
    }

    fn roots(&self, _root: &Path, _repo: &Repo) -> Vec<PathBuf> {
        Vec::new()
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        for (name, line) in trees::texts(&language(), USES, &tree, source, "typeref") {
            self.typerefs.push((file.to_path_buf(), name, line));
        }
        for (path, _) in trees::texts(&language(), USES, &tree, source, "import") {
            self.imports
                .push((file.to_path_buf(), path.trim_matches('"').to_string()));
        }
        if !surface {
            return Vec::new();
        }
        let mentions = mentions(&tree, source);
        definitions(&tree, source)
            .into_iter()
            .map(|(name, line)| Definition {
                self_used: mentions
                    .iter()
                    .any(|(mentioned, at)| *mentioned == name && *at != line),
                name,
                line,
            })
            .collect()
    }

    // A type reference reaches a definition in its own file, or in a file it imports.
    fn resolve(&self, mine: usize, defs: &[Def], _root: &Path, repo: &Repo) -> Vec<Edge> {
        let mut edges = Vec::new();
        for (file, reference, line) in &self.typerefs {
            let name = reference.rsplit('.').next().unwrap_or(reference);
            for (index, def) in defs.iter().enumerate() {
                if def.pack != mine || def.name != name || def.file == *file {
                    continue;
                }
                let imported = self
                    .imports
                    .iter()
                    .any(|(from, path)| from == file && def.file.to_string_lossy().ends_with(path));
                if imported {
                    edges.push(Edge {
                        def: index,
                        file: file.clone(),
                        line: *line,
                        unit: repo.unit_of(file),
                    });
                }
            }
        }
        edges
    }

    fn suppressions(&self) -> &[Suppression] {
        &[]
    }

    fn drift(
        &self,
        root: &Path,
        folder: &Path,
        fixing: bool,
        notes: &mut Vec<String>,
    ) -> Vec<Drift> {
        let mut found = Vec::new();
        if folder.join("buf.gen.yaml").is_file() {
            let started = Instant::now();
            let problems = regenerated(folder, fixing);
            found.push(Drift {
                check: "drift".to_string(),
                problems,
                seconds: started.elapsed().as_secs_f64(),
            });
        }
        if folder.join("buf.yaml").is_file() {
            let image = image_path(root, folder);
            if image.is_file() {
                let started = Instant::now();
                let problems =
                    match run_buf(folder, &["breaking", "--against", &image.to_string_lossy()]) {
                        Ok(_) => Vec::new(),
                        Err(output) => output.lines().map(str::to_string).collect(),
                    };
                found.push(Drift {
                    check: "breaking".to_string(),
                    problems,
                    seconds: started.elapsed().as_secs_f64(),
                });
            } else {
                let shown = folder.strip_prefix(root).unwrap_or(folder);
                notes.push(format!(
                    "{} has no contracts reference; arbiter accept records today's",
                    shown.to_string_lossy()
                ));
            }
        }
        found
    }

    fn record(&self, root: &Path, repo: &Repo) -> Vec<String> {
        let mut lines = Vec::new();
        for manifest in &repo.manifests {
            if manifest.file_name().is_none_or(|name| name != "buf.yaml") {
                continue;
            }
            let unit = manifest.parent().unwrap_or(Path::new(""));
            // A unit on a test path is a fixture: data for some test, not contracts to record.
            if repo.is_test(repo.unit_of(manifest), unit) {
                continue;
            }
            let folder = root.join(unit);
            let image = image_path(root, &folder);
            if let Some(parent) = image.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let kept = image.strip_prefix(root).unwrap_or(&image).to_path_buf();
            match run_buf(&folder, &["build", "-o", &image.to_string_lossy()]) {
                Ok(_) => lines.push(format!(
                    "contracts of {} recorded in {}",
                    unit.to_string_lossy(),
                    kept.to_string_lossy()
                )),
                Err(output) => lines.push(format!(
                    "contracts of {} could not be recorded: {output}",
                    unit.to_string_lossy()
                )),
            }
        }
        lines
    }
}

// Where the accepted contracts of the unit live, named by the unit's own path.
fn image_path(root: &Path, folder: &Path) -> PathBuf {
    let unit = folder.strip_prefix(root).unwrap_or(folder);
    let flat = if unit.as_os_str().is_empty() {
        "root".to_string()
    } else {
        unit.to_string_lossy().replace(['/', '\\'], "-")
    };
    root.join(".arbiter")
        .join("contracts")
        .join(format!("{flat}.binpb"))
}

// Success keeps nothing; failure keeps the tool's whole answer.
fn run_buf(folder: &Path, args: &[&str]) -> Result<(), String> {
    let finished = std::process::Command::new(BUF[0])
        .args(&BUF[1..])
        .args(args)
        .current_dir(folder)
        .output();
    let Ok(output) = finished else {
        return Err(format!("{} did not start", BUF[0]));
    };
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .trim()
    .to_string())
}

// Regenerates to a staging folder and names every committed file that disagrees.
// A fixing run first regenerates in place and deletes the orphans.
fn regenerated(folder: &Path, fixing: bool) -> Vec<String> {
    if fixing {
        let _ = run_buf(folder, &["generate"]);
    }
    let staged = std::env::temp_dir()
        .join("arbiter-gen")
        .join(folder.to_string_lossy().replace(['/', '\\', ':'], "-"));
    let _ = std::fs::remove_dir_all(&staged);
    if std::fs::create_dir_all(&staged).is_err() {
        return vec!["the staging folder could not be created".to_string()];
    }
    if let Err(output) = run_buf(folder, &["generate", "--output", &staged.to_string_lossy()]) {
        return vec![format!("buf generate failed: {output}")];
    }
    if fixing {
        for orphan in orphans(&staged, folder) {
            let _ = std::fs::remove_file(folder.join(orphan));
        }
    }
    compared(&staged, folder)
}

fn compared(generated: &Path, committed: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    for file in files_under(generated, generated) {
        let theirs = std::fs::read(generated.join(&file)).unwrap_or_default();
        match std::fs::read(committed.join(&file)) {
            Err(_) => problems.push(format!(
                "{} is missing; the contracts generate it",
                file.to_string_lossy()
            )),
            Ok(ours) if ours != theirs => problems.push(format!(
                "{} drifted from what the contracts generate",
                file.to_string_lossy()
            )),
            Ok(_) => {}
        }
    }
    for orphan in orphans(generated, committed) {
        problems.push(format!(
            "{} is orphaned; no contract generates it",
            orphan.to_string_lossy()
        ));
    }
    problems.sort();
    problems
}

// Committed files named like generated code that no contract generates any more.
fn orphans(generated: &Path, committed: &Path) -> Vec<PathBuf> {
    let fresh: std::collections::HashSet<PathBuf> =
        files_under(generated, generated).into_iter().collect();
    files_under(committed, committed)
        .into_iter()
        .filter(|file| {
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            GENERATED.iter().any(|suffix| name.ends_with(suffix)) && !fresh.contains(file)
        })
        .collect()
}

fn files_under(base: &Path, folder: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return found;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        if kind.is_dir() {
            if name.starts_with('.')
                || ["node_modules", "dist", "target", "__pycache__"].contains(&name.as_str())
            {
                continue;
            }
            found.extend(files_under(base, &path));
        } else if kind.is_file() {
            found.push(path.strip_prefix(base).unwrap_or(&path).to_path_buf());
        }
    }
    found
}

fn language() -> Language {
    tree_sitter_proto::LANGUAGE.into()
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), DEFINITIONS, tree, source, "def")
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), MENTIONS, tree, source, "mention")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::compared;

    fn planted(base: &Path, file: &str, content: &str) {
        let path = base.join(file);
        fs::create_dir_all(path.parent().expect("a planted file has a folder"))
            .expect("the folder is writable");
        fs::write(path, content).expect("the file is writable");
    }

    #[test]
    fn a_missing_a_drifted_and_an_orphaned_file_are_each_named() {
        let base = std::env::temp_dir().join("arbiter-test-compared");
        let _ = fs::remove_dir_all(&base);
        let generated = base.join("generated");
        let committed = base.join("committed");
        planted(&generated, "src/gen/same_pb.ts", "same");
        planted(&generated, "src/gen/stale_pb.ts", "new");
        planted(&generated, "src/gen/fresh_pb.ts", "fresh");
        planted(&committed, "src/gen/same_pb.ts", "same");
        planted(&committed, "src/gen/stale_pb.ts", "old");
        planted(&committed, "src/gen/ghost_pb.ts", "ghost");
        planted(&committed, "src/own.ts", "handwritten");
        let problems = compared(&generated, &committed);
        assert_eq!(
            problems,
            [
                "src/gen/fresh_pb.ts is missing; the contracts generate it",
                "src/gen/ghost_pb.ts is orphaned; no contract generates it",
                "src/gen/stale_pb.ts drifted from what the contracts generate",
            ]
        );
    }
}
