use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use tree_sitter::{Language, Node};

use crate::pack::Pack;
use crate::trees;

pub struct Unit {
    pub path: PathBuf,
    pub test_files: HashSet<PathBuf>,
    pub root_files: HashSet<PathBuf>,
}

pub struct Repo {
    pub units: Vec<Unit>,
    pub files: Vec<PathBuf>,
    pub manifests: Vec<PathBuf>,
}

impl Repo {
    // The BUILD file's word when it gives one; the naming convention where no target claims the file.
    pub fn is_test(&self, unit: usize, file: &Path) -> bool {
        if self.units[unit].test_files.contains(file) {
            return true;
        }
        let stem = file
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default();
        stem.starts_with("test_")
            || stem.ends_with("_test")
            || stem == "conftest"
            || file.components().any(|part| part.as_os_str() == "tests")
    }

    pub fn unit_of(&self, file: &Path) -> usize {
        self.units
            .iter()
            .enumerate()
            .filter(|(_, unit)| file.starts_with(&unit.path))
            .max_by_key(|(_, unit)| unit.path.components().count())
            .map(|(index, _)| index)
            .unwrap_or(0)
    }
}

const SKIPPED: [&str; 2] = ["node_modules", "target"];
const BUILD_NAMES: [&str; 3] = ["BUILD.bazel", "BUILD.bazel.held", "BUILD"];
const MANIFEST_NAMES: [&str; 4] = ["pyproject.toml", "Cargo.toml", "package.json", "go.mod"];

pub fn scan(root: &Path, packs: &[Box<dyn Pack>]) -> Repo {
    let mut builds = Vec::new();
    let mut files = Vec::new();
    let mut manifests = Vec::new();
    walk(root, root, packs, &mut builds, &mut files, &mut manifests);
    // The whole repository is the unit of last resort for a file no BUILD file claims.
    let mut units = vec![Unit {
        path: PathBuf::new(),
        test_files: HashSet::new(),
        root_files: HashSet::new(),
    }];
    for build in builds {
        let folder = build.parent().unwrap_or(Path::new("")).to_path_buf();
        let unit = read_unit(root, &build, folder);
        if unit.path.as_os_str().is_empty() {
            units[0] = unit;
        } else {
            units.push(unit);
        }
    }
    files.sort();
    Repo {
        units,
        files,
        manifests,
    }
}

fn walk(
    root: &Path,
    folder: &Path,
    packs: &[Box<dyn Pack>],
    builds: &mut Vec<PathBuf>,
    files: &mut Vec<PathBuf>,
    manifests: &mut Vec<PathBuf>,
) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        if kind.is_dir() {
            if name.starts_with('.') || name.starts_with("bazel-") || SKIPPED.contains(&name.as_str()) {
                continue;
            }
            walk(root, &path, packs, builds, files, manifests);
        } else if kind.is_file() {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            if BUILD_NAMES.contains(&name.as_str()) {
                builds.push(relative);
            } else if MANIFEST_NAMES.contains(&name.as_str()) {
                manifests.push(relative);
            } else if packs.iter().any(|pack| pack.handles(&name)) {
                files.push(relative);
            }
        }
    }
}

// Starlark is a subset of Python's syntax, so the Python grammar reads a BUILD file.
fn read_unit(root: &Path, build: &Path, folder: PathBuf) -> Unit {
    let mut unit = Unit {
        path: folder,
        test_files: HashSet::new(),
        root_files: HashSet::new(),
    };
    let Ok(source) = fs::read_to_string(root.join(build)) else {
        return unit;
    };
    let language: Language = tree_sitter_python::LANGUAGE.into();
    let Some(tree) = trees::parse(&language, &source) else {
        return unit;
    };
    let top = tree.root_node();
    let mut walker = top.walk();
    for statement in top.children(&mut walker) {
        let Some((kind, srcs, main)) = target(statement, &source) else {
            continue;
        };
        if kind.ends_with("_test") {
            unit.test_files
                .extend(srcs.iter().map(|src| unit.path.join(src)));
        } else if kind.ends_with("_binary") {
            // A binary's entry file is a root: reachability starts there, not at a caller.
            let roots = main.map(|file| vec![file]).unwrap_or(srcs);
            unit.root_files
                .extend(roots.iter().map(|src| unit.path.join(src)));
        }
    }
    unit
}

fn target(statement: Node, source: &str) -> Option<(String, Vec<String>, Option<String>)> {
    if statement.kind() != "expression_statement" {
        return None;
    }
    let call = statement.child(0).filter(|node| node.kind() == "call")?;
    let function = call.child_by_field_name("function")?;
    if function.kind() != "identifier" {
        return None;
    }
    let kind = text(function, source);
    if kind == "load" {
        return None;
    }
    let arguments = call.child_by_field_name("arguments")?;
    let mut srcs = Vec::new();
    let mut main = None;
    let mut walker = arguments.walk();
    for argument in arguments.children(&mut walker) {
        if argument.kind() != "keyword_argument" {
            continue;
        }
        let Some(name) = argument.child_by_field_name("name") else {
            continue;
        };
        let Some(value) = argument.child_by_field_name("value") else {
            continue;
        };
        match text(name, source).as_str() {
            "srcs" => srcs = strings(value, source),
            "main" => main = strings(value, source).into_iter().next(),
            _ => {}
        }
    }
    Some((kind, srcs, main))
}

fn strings(node: Node, source: &str) -> Vec<String> {
    match node.kind() {
        "string" => vec![unquoted(&text(node, source))],
        "list" => {
            let mut walker = node.walk();
            node.children(&mut walker)
                .filter(|child| child.kind() == "string")
                .map(|child| unquoted(&text(child, source)))
                .collect()
        }
        _ => Vec::new(),
    }
}

fn unquoted(quoted: &str) -> String {
    quoted.trim_matches(['"', '\'']).to_string()
}

fn text(node: Node, source: &str) -> String {
    node.utf8_text(source.as_bytes()).unwrap_or("").to_string()
}
