use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tree_sitter::{Language, Node};

use crate::core::pack::Pack;
use crate::core::trees;

pub struct Unit {
    pub path: PathBuf,
    pub test_files: HashSet<PathBuf>,
    pub root_files: HashSet<PathBuf>,
}

pub struct Repo {
    pub units: Vec<Unit>,
    pub files: Vec<PathBuf>,
    pub manifests: Vec<PathBuf>,
    pub lockfiles: Vec<PathBuf>,
    // Folders holding a PUBLISHED marker: that unit's surface serves users
    // the repository cannot see.
    pub published: HashSet<PathBuf>,
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
            || stem.ends_with(".test")
            || stem == "conftest"
            || file
                .components()
                .any(|part| part.as_os_str() == "tests" || part.as_os_str() == "__tests__")
    }

    pub fn is_published(&self, unit: usize) -> bool {
        self.published.contains(&self.units[unit].path)
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

const BUILD_NAMES: [&str; 3] = ["BUILD.bazel", "BUILD.bazel.held", "BUILD"];
const MANIFEST_NAMES: [&str; 6] = [
    "pyproject.toml",
    "Cargo.toml",
    "package.json",
    "go.mod",
    "buf.yaml",
    "buf.work.yaml",
];

pub fn scan(root: &Path, packs: &[Box<dyn Pack>]) -> Repo {
    let mut builds = Vec::new();
    let mut found = Found {
        files: Vec::new(),
        manifests: Vec::new(),
        lockfiles: Vec::new(),
        published: HashSet::new(),
    };
    let skips = ignored(root);
    walk(root, root, packs, &skips, &mut builds, &mut found);
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
    // A manifest declares a unit the way a BUILD file does, with nothing to add to it.
    for manifest in &found.manifests {
        let folder = manifest.parent().unwrap_or(Path::new("")).to_path_buf();
        if folder.as_os_str().is_empty() || units.iter().any(|unit| unit.path == folder) {
            continue;
        }
        units.push(Unit {
            path: folder,
            test_files: HashSet::new(),
            root_files: HashSet::new(),
        });
    }
    found.files.sort();
    Repo {
        units,
        files: found.files,
        manifests: found.manifests,
        lockfiles: found.lockfiles,
        published: found.published,
    }
}

struct Found {
    files: Vec<PathBuf>,
    manifests: Vec<PathBuf>,
    lockfiles: Vec<PathBuf>,
    published: HashSet<PathBuf>,
}

// What git ignores is not the repository: a judged file is one git would carry.
// A committed file stays judged whatever an ignore pattern says, and a folder
// with no git at all ignores nothing.
fn ignored(root: &Path) -> HashSet<PathBuf> {
    let listed = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "-z",
        ])
        .output();
    let Ok(listed) = listed else {
        return HashSet::new();
    };
    if !listed.status.success() {
        return HashSet::new();
    }
    String::from_utf8_lossy(&listed.stdout)
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(path.trim_end_matches('/')))
        .collect()
}

fn walk(
    root: &Path,
    folder: &Path,
    packs: &[Box<dyn Pack>],
    skips: &HashSet<PathBuf>,
    builds: &mut Vec<PathBuf>,
    found: &mut Found,
) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        if skips.contains(&relative) {
            continue;
        }
        if kind.is_dir() {
            // A third_party folder holds vendored bytes: read by builds, never judged.
            if name.starts_with('.')
                || name.starts_with("bazel-")
                || name == "third_party"
                || packs
                    .iter()
                    .any(|pack| pack.outputs().contains(&name.as_str()))
            {
                continue;
            }
            walk(root, &path, packs, skips, builds, found);
        } else if kind.is_file() {
            if name == "PUBLISHED" {
                let parent = relative.parent().unwrap_or(Path::new("")).to_path_buf();
                found.published.insert(parent);
            } else if BUILD_NAMES.contains(&name.as_str()) {
                builds.push(relative);
            } else if MANIFEST_NAMES.contains(&name.as_str()) {
                found.manifests.push(relative);
            } else if packs
                .iter()
                .any(|pack| pack.lockfiles().contains(&name.as_str()))
            {
                found.lockfiles.push(relative);
            } else if packs.iter().any(|pack| pack.handles(&name)) {
                found.files.push(relative);
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    use super::{Repo, Unit};

    fn repo(paths: &[&str]) -> Repo {
        Repo {
            units: paths
                .iter()
                .map(|path| Unit {
                    path: PathBuf::from(path),
                    test_files: HashSet::new(),
                    root_files: HashSet::new(),
                })
                .collect(),
            files: Vec::new(),
            manifests: Vec::new(),
            lockfiles: Vec::new(),
            published: HashSet::new(),
        }
    }

    #[test]
    fn the_deepest_unit_claims_the_file() {
        let repo = repo(&["", "libs", "libs/a"]);
        assert_eq!(repo.unit_of(Path::new("libs/a/src/x.py")), 2);
        assert_eq!(repo.unit_of(Path::new("libs/b/src/x.py")), 1);
        assert_eq!(repo.unit_of(Path::new("apps/x.py")), 0);
    }

    #[test]
    fn a_test_is_told_by_its_name_or_its_folder() {
        let repo = repo(&[""]);
        assert!(repo.is_test(0, Path::new("tests/anything.py")));
        assert!(repo.is_test(0, Path::new("src/test_calc.py")));
        assert!(repo.is_test(0, Path::new("src/calc_test.cc")));
        assert!(repo.is_test(0, Path::new("conftest.py")));
        assert!(repo.is_test(0, Path::new("src/__tests__/calc.ts")));
        assert!(repo.is_test(0, Path::new("src/calc.test.ts")));
        assert!(!repo.is_test(0, Path::new("src/calc.py")));
        assert!(!repo.is_test(0, Path::new("src/latest.ts")));
    }
}
