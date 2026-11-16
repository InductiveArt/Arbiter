use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::ecosystems::ecosystems;
use crate::core::listing::committed::committed;
use crate::core::listing::list;
use crate::core::markers::read_markers;
use crate::core::pack::Pack;
use crate::core::repository::Repo;
use crate::domain::file::File;
use crate::domain::marker::Marker;
use crate::domain::unit::Unit;

// The repository under a folder as the languages find it: its files, the
// units their markers make, and what a law asks of the files by name: the
// manifests, the lockfiles and the PUBLISHED marks.
pub fn scan(root: &Path, packs: &[Box<dyn Pack>]) -> Result<Repo, String> {
    let outputs: Vec<&str> = packs
        .iter()
        .flat_map(|pack| pack.outputs().iter().copied())
        .collect();
    let carried = committed(root);
    let listing = list(root, &outputs, &carried, &mut || Ok(()))?;
    let markers = read_markers(root, &listing.files, &ecosystems(packs)?, &mut || Ok(()))?;
    let units = group_files_by_unit(&listing.files, &markers);
    let lockfiles: HashSet<&str> = packs
        .iter()
        .flat_map(|pack| pack.lockfiles().iter().copied())
        .collect();
    let named = |file: &PathBuf, names: &HashSet<&str>| {
        file.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| names.contains(name))
    };
    let files: Vec<PathBuf> = listing.files.into_iter().map(|file| file.path).collect();
    let manifests: BTreeSet<PathBuf> = markers.into_iter().map(|marker| marker.path).collect();
    let published = files
        .iter()
        .filter(|file| file.file_name().is_some_and(|name| name == "PUBLISHED"))
        .map(|file| file.parent().unwrap_or(Path::new("")).to_path_buf())
        .collect();
    Ok(Repo {
        units,
        roots: HashSet::new(),
        lockfiles: files
            .iter()
            .filter(|file| named(file, &lockfiles))
            .cloned()
            .collect(),
        manifests: manifests.into_iter().collect(),
        published,
        foreign: listing.foreign,
        files,
    })
}

// Every unit, sorted by path, the listed folder itself first. A unit is the
// listed folder or any folder under it holding a marker, and a file goes to
// the deepest unit above it, so each listed file is under exactly one. No
// disk is touched and no file name is weighed: each marker names its own
// path, so the markers alone say where the units are.
pub fn group_files_by_unit(files: &[File], markers: &[Marker]) -> Vec<Unit> {
    let whole = Path::new("");
    let mut units: BTreeMap<PathBuf, Unit> = BTreeMap::new();
    units.insert(whole.to_path_buf(), unit(whole));
    for marker in markers {
        let folder = marker.path.parent().unwrap_or(whole);
        units
            .entry(folder.to_path_buf())
            .or_insert_with(|| unit(folder))
            .markers
            .push(marker.clone());
    }
    for file in files {
        let folder = file
            .path
            .ancestors()
            .skip(1)
            .find(|above| units.contains_key(*above))
            .unwrap_or(whole);
        if let Some(unit) = units.get_mut(folder) {
            unit.files.push(file.clone());
        }
    }
    units.into_values().collect()
}

fn unit(folder: &Path) -> Unit {
    Unit {
        path: folder.to_path_buf(),
        markers: Vec::new(),
        files: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::group_files_by_unit;
    use crate::domain::file::File;
    use crate::domain::marker::Marker;
    use crate::domain::unit::Unit;

    fn list(paths: &[&str]) -> Vec<File> {
        paths
            .iter()
            .map(|path| File {
                path: PathBuf::from(path),
                bytes: path.len() as u64,
                digest: format!("digest of {path}"),
            })
            .collect()
    }

    // Each marker by its path and the ecosystem it marks for.
    fn markers(told: &[(&str, &str)]) -> Vec<Marker> {
        told.iter()
            .map(|(path, ecosystem)| Marker {
                path: PathBuf::from(path),
                ecosystem: ecosystem.to_string(),
                members_only: false,
            })
            .collect()
    }

    // Each unit as its path, the ecosystems of its markers and its files'
    // paths.
    fn told(units: &[Unit]) -> Vec<(String, Vec<String>, Vec<String>)> {
        units
            .iter()
            .map(|unit| {
                (
                    unit.path.to_string_lossy().into_owned(),
                    unit.markers
                        .iter()
                        .map(|marker| marker.ecosystem.clone())
                        .collect(),
                    unit.files
                        .iter()
                        .map(|file| file.path.to_string_lossy().into_owned())
                        .collect(),
                )
            })
            .collect()
    }

    fn unit(path: &str, ecosystems: &[&str], files: &[&str]) -> (String, Vec<String>, Vec<String>) {
        (
            path.to_string(),
            ecosystems.iter().map(|name| name.to_string()).collect(),
            files.iter().map(|file| file.to_string()).collect(),
        )
    }

    #[test]
    fn the_listed_folder_is_a_unit_even_with_no_marker_and_holds_what_no_other_takes() {
        let units = group_files_by_unit(&list(&["README.md", "docs/a.md"]), &markers(&[]));
        assert_eq!(told(&units), [unit("", &[], &["README.md", "docs/a.md"])]);
        let empty = group_files_by_unit(&list(&[]), &markers(&[]));
        assert_eq!(told(&empty), [unit("", &[], &[])]);
    }

    #[test]
    fn a_folder_holding_a_marker_is_a_unit_and_its_files_leave_the_one_above() {
        let listed = list(&[
            "README.md",
            "libs/a/Cargo.toml",
            "libs/a/src/lib.rs",
            "libs/b.txt",
        ]);
        let units = group_files_by_unit(&listed, &markers(&[("libs/a/Cargo.toml", "rust")]));
        assert_eq!(
            told(&units),
            [
                unit("", &[], &["README.md", "libs/b.txt"]),
                unit(
                    "libs/a",
                    &["rust"],
                    &["libs/a/Cargo.toml", "libs/a/src/lib.rs"]
                ),
            ]
        );
        assert_eq!(
            units[1].files[1], listed[2],
            "a file keeps its size and digest"
        );
    }

    #[test]
    fn a_file_goes_to_the_deepest_unit_above_it() {
        let listed = list(&[
            "Cargo.toml",
            "src/lib.rs",
            "tools/x/Cargo.toml",
            "tools/x/inner/a.py",
            "tools/x/inner/pyproject.toml",
            "tools/x/src/main.rs",
        ]);
        let marked = markers(&[
            ("Cargo.toml", "rust"),
            ("tools/x/Cargo.toml", "rust"),
            ("tools/x/inner/pyproject.toml", "python"),
        ]);
        let units = group_files_by_unit(&listed, &marked);
        assert_eq!(
            told(&units),
            [
                unit("", &["rust"], &["Cargo.toml", "src/lib.rs"]),
                unit(
                    "tools/x",
                    &["rust"],
                    &["tools/x/Cargo.toml", "tools/x/src/main.rs"]
                ),
                unit(
                    "tools/x/inner",
                    &["python"],
                    &["tools/x/inner/a.py", "tools/x/inner/pyproject.toml"]
                ),
            ]
        );
        let held: usize = units.iter().map(|unit| unit.files.len()).sum();
        assert_eq!(held, listed.len(), "each file is under exactly one");
    }

    #[test]
    fn a_folder_holding_two_markers_is_one_unit_holding_both() {
        let marked = markers(&[("app/Cargo.toml", "rust"), ("app/pyproject.toml", "python")]);
        let units = group_files_by_unit(&list(&["app/Cargo.toml", "app/pyproject.toml"]), &marked);
        assert_eq!(
            told(&units),
            [
                unit("", &[], &[]),
                unit(
                    "app",
                    &["rust", "python"],
                    &["app/Cargo.toml", "app/pyproject.toml"]
                ),
            ]
        );
    }

    #[test]
    fn a_unit_holds_each_of_its_markers_whole() {
        let only_lists = Marker {
            path: PathBuf::from("ws/Cargo.toml"),
            ecosystem: "rust".to_string(),
            members_only: true,
        };
        let marked = vec![only_lists.clone()];
        let units = group_files_by_unit(&list(&["ws/Cargo.toml"]), &marked);
        assert_eq!(units.len(), 2);
        assert_eq!(units[1].markers, [only_lists]);
    }

    #[test]
    fn a_file_named_as_a_marker_makes_no_unit_when_no_marker_tells_it() {
        let units = group_files_by_unit(&list(&["a/Cargo.toml", "a/src/lib.rs"]), &markers(&[]));
        assert_eq!(
            told(&units),
            [unit("", &[], &["a/Cargo.toml", "a/src/lib.rs"])]
        );
    }
}
