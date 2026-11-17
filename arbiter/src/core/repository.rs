use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::domain::unit::Unit;

#[derive(Clone, Serialize, Deserialize)]
pub struct Repo {
    pub units: Vec<Unit>,
    // The files whose symbols live by being named from outside the repository.
    pub roots: BTreeSet<PathBuf>,
    pub files: Vec<PathBuf>,
    pub manifests: Vec<PathBuf>,
    pub lockfiles: Vec<PathBuf>,
    // Folders holding a PUBLISHED marker: that unit's surface serves users
    // the repository cannot see.
    pub published: BTreeSet<PathBuf>,
    // Folders holding their own .arbiter: other repositories, gated from
    // their own root. The scan names each one and never crosses its border.
    pub foreign: Vec<PathBuf>,
}

impl Repo {
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

    // The name a declaration uses for the unit: its path, or "." at the root.
    pub fn unit_name(&self, unit: usize) -> String {
        let path = &self.units[unit].path;
        if path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            path.to_string_lossy().to_string()
        }
    }
}

// A test by the naming convention: the file's own name, or a folder it sits in.
pub fn is_test(file: &Path) -> bool {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;
    use std::path::PathBuf;

    use super::is_test;
    use super::Repo;
    use crate::domain::unit::Unit;

    fn repo(paths: &[&str]) -> Repo {
        Repo {
            units: paths
                .iter()
                .map(|path| Unit {
                    path: PathBuf::from(path),
                    markers: Vec::new(),
                    files: Vec::new(),
                })
                .collect(),
            roots: BTreeSet::new(),
            files: Vec::new(),
            manifests: Vec::new(),
            lockfiles: Vec::new(),
            published: BTreeSet::new(),
            foreign: Vec::new(),
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
        assert!(is_test(Path::new("tests/anything.py")));
        assert!(is_test(Path::new("src/test_calc.py")));
        assert!(is_test(Path::new("src/calc_test.cc")));
        assert!(is_test(Path::new("conftest.py")));
        assert!(is_test(Path::new("src/__tests__/calc.ts")));
        assert!(is_test(Path::new("src/calc.test.ts")));
        assert!(!is_test(Path::new("src/calc.py")));
        assert!(!is_test(Path::new("src/latest.ts")));
    }
}
