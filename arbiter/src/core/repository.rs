use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::domain::unit::Unit;

pub struct Repo {
    pub units: Vec<Unit>,
    // The files whose symbols live by being named from outside the repository.
    pub roots: HashSet<PathBuf>,
    pub files: Vec<PathBuf>,
    pub manifests: Vec<PathBuf>,
    pub lockfiles: Vec<PathBuf>,
    // Folders holding a PUBLISHED marker: that unit's surface serves users
    // the repository cannot see.
    pub published: HashSet<PathBuf>,
    // Folders holding their own .arbiter: other repositories, gated from
    // their own root. The scan names each one and never crosses its border.
    pub foreign: Vec<PathBuf>,
}

impl Repo {
    // A test by the naming convention, the test's file or the folder it sits in.
    pub fn is_test(&self, unit: usize, file: &Path) -> bool {
        let _ = unit;
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;
    use std::path::PathBuf;

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
            roots: HashSet::new(),
            files: Vec::new(),
            manifests: Vec::new(),
            lockfiles: Vec::new(),
            published: HashSet::new(),
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
