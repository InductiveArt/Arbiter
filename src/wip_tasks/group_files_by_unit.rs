use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::wip::pull::In;
use crate::wip::scope::Scope;
use crate::wip::task::Task;
use crate::wip::trace::outcome::Failure;
use crate::wip_tasks::list_files::File;
use crate::wip_tasks::list_files::Files;
use crate::wip_tasks::read_markers::Marker;
use crate::wip_tasks::read_markers::Markers;

// Every unit, sorted by path, the listed folder itself first. Each listed
// file is under exactly one.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Units {
    pub units: Vec<Unit>,
}

// One unit: its folder by path from the listed folder, empty for that folder
// itself, the markers it holds, and its files under the paths they were
// listed by.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Unit {
    pub path: PathBuf,
    pub markers: Vec<Marker>,
    pub files: Vec<File>,
}

pub fn group_files_by_unit_task() -> Task {
    Task::new(
        "group_files_by_unit",
        |scope: &mut Scope, (files, markers): (In<Files>, In<Markers>)| {
            let units = group_files_by_unit(&files, &markers);
            scope.measure("units", units.units.len() as f64);
            scope.measure("files", files.files.len() as f64);
            Ok::<Units, Failure>(units)
        },
    )
    .line(
        "each unit with its own files: a unit is the listed folder or any folder under it \
         holding a marker, and a file goes to the deepest unit above it",
    )
}

// No disk is touched and no file name is weighed: each marker names its own
// path, so the markers alone say where the units are.
fn group_files_by_unit(files: &Files, markers: &Markers) -> Units {
    let whole = Path::new("");
    let mut units: BTreeMap<PathBuf, Unit> = BTreeMap::new();
    units.insert(whole.to_path_buf(), unit(whole));
    for marker in &markers.markers {
        let folder = marker.path.parent().unwrap_or(whole);
        units
            .entry(folder.to_path_buf())
            .or_insert_with(|| unit(folder))
            .markers
            .push(marker.clone());
    }
    for file in &files.files {
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
    Units {
        units: units.into_values().collect(),
    }
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
    use std::env;
    use std::path::PathBuf;

    use super::group_files_by_unit;
    use super::group_files_by_unit_task;
    use super::Units;
    use crate::wip::job::cancel::Cancel;
    use crate::wip::job::id::JobId;
    use crate::wip::job::id::RunId;
    use crate::wip::layout::Site;
    use crate::wip::scope::Scope;
    use crate::wip::store::Part;
    use crate::wip::store::Product;
    use crate::wip::trace::outcome::Outcome;
    use crate::wip_tasks::list_files::File;
    use crate::wip_tasks::list_files::Files;
    use crate::wip_tasks::read_markers::Marker;
    use crate::wip_tasks::read_markers::Markers;

    fn list(paths: &[&str]) -> Files {
        Files {
            files: paths
                .iter()
                .map(|path| File {
                    path: PathBuf::from(path),
                    bytes: path.len() as u64,
                    digest: format!("digest of {path}"),
                })
                .collect(),
        }
    }

    // Each marker by its path and the ecosystem it marks for.
    fn markers(told: &[(&str, &str)]) -> Markers {
        Markers {
            markers: told
                .iter()
                .map(|(path, ecosystem)| Marker {
                    path: PathBuf::from(path),
                    ecosystem: ecosystem.to_string(),
                    members_only: false,
                })
                .collect(),
        }
    }

    // Each unit as its path, the ecosystems of its markers and its files'
    // paths.
    fn told(units: &Units) -> Vec<(String, Vec<String>, Vec<String>)> {
        units
            .units
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
            units.units[1].files[1], listed.files[2],
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
        let held: usize = units.units.iter().map(|unit| unit.files.len()).sum();
        assert_eq!(held, listed.files.len(), "each file is under exactly one");
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
        let marked = Markers {
            markers: vec![only_lists.clone()],
        };
        let units = group_files_by_unit(&list(&["ws/Cargo.toml"]), &marked);
        assert_eq!(units.units.len(), 2);
        assert_eq!(units.units[1].markers, [only_lists]);
    }

    #[test]
    fn a_file_named_as_a_marker_makes_no_unit_when_no_marker_tells_it() {
        let units = group_files_by_unit(&list(&["a/Cargo.toml", "a/src/lib.rs"]), &markers(&[]));
        assert_eq!(
            told(&units),
            [unit("", &[], &["a/Cargo.toml", "a/src/lib.rs"])]
        );
    }

    #[test]
    fn the_task_reads_the_list_and_the_markers_and_measures_what_it_leaves() {
        let site = Site {
            dir: env::temp_dir(),
            store: env::temp_dir(),
        };
        let mut scope = Scope::new(
            JobId::new(RunId::next(), "group_files_by_unit", 1),
            Cancel::new(),
            site,
        );
        let parts = vec![
            Part {
                value: Some(
                    Product::encoded(&list(&["a/Cargo.toml", "a/x.rs", "b.md"]))
                        .expect("a list encodes"),
                ),
                completion: None,
            },
            Part {
                value: Some(
                    Product::encoded(&markers(&[("a/Cargo.toml", "rust")]))
                        .expect("markers encode"),
                ),
                completion: None,
            },
        ];
        let bytes = group_files_by_unit_task()
            .perform(&mut scope, parts)
            .expect("the list is grouped");
        let units: Units = serde_json::from_slice(&bytes).expect("the product decodes");
        assert_eq!(
            told(&units),
            [
                unit("", &[], &["b.md"]),
                unit("a", &["rust"], &["a/Cargo.toml", "a/x.rs"]),
            ]
        );
        let step = scope.finished("group_files_by_unit", Outcome::Ok);
        assert_eq!(step.measures.get("units"), Some(&2.0));
        assert_eq!(step.measures.get("files"), Some(&3.0));
    }
}
