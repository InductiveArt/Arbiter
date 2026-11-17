use std::path::Path;
use std::path::PathBuf;

// The folders Cargo makes a crate of each file in.
const TARGET_FOLDERS: [&str; 3] = ["tests", "examples", "benches"];

// Where a crate finds its modules, as Cargo lays a package out: the library
// and the main binary under src, every other binary, test, example and bench
// a crate of its own; and the file it was read for, as a module path from
// that crate's root.
pub struct CrateLayout {
    pub dir: PathBuf,
    // The files that can be the crate's root module.
    pub roots: Vec<PathBuf>,
    pub own: Vec<String>,
}

impl CrateLayout {
    pub fn of(unit: &Path, file: &Path) -> CrateLayout {
        let inside: Vec<String> = file
            .strip_prefix(unit)
            .unwrap_or(file)
            .iter()
            .map(|part| part.to_string_lossy().into_owned())
            .collect();
        let parts: Vec<&str> = inside.iter().map(String::as_str).collect();
        match parts.as_slice() {
            ["src", "bin", _] => CrateLayout {
                dir: unit.join("src").join("bin"),
                roots: vec![file.to_path_buf()],
                own: Vec::new(),
            },
            ["src", "bin", binary, rest @ ..] => {
                let dir = unit.join("src").join("bin").join(binary);
                CrateLayout {
                    roots: vec![dir.join("main.rs")],
                    own: module_path(rest),
                    dir,
                }
            }
            ["src", rest @ ..] => {
                let dir = unit.join("src");
                CrateLayout {
                    roots: vec![dir.join("lib.rs"), dir.join("main.rs")],
                    own: module_path(rest),
                    dir,
                }
            }
            [folder, _] if TARGET_FOLDERS.contains(folder) => CrateLayout {
                dir: unit.join(folder),
                roots: vec![file.to_path_buf()],
                own: Vec::new(),
            },
            // A module the folder's crates share, as tests/common/mod.rs is.
            [folder, rest @ ..] if TARGET_FOLDERS.contains(folder) => CrateLayout {
                dir: unit.join(folder),
                roots: Vec::new(),
                own: module_path(rest),
            },
            _ => CrateLayout {
                dir: file.parent().map(Path::to_path_buf).unwrap_or_default(),
                roots: vec![file.to_path_buf()],
                own: Vec::new(),
            },
        }
    }

    // A crate of the repository as another reaches it by name: its library.
    pub fn library(unit: &Path) -> CrateLayout {
        let dir = unit.join("src");
        CrateLayout {
            roots: vec![dir.join("lib.rs")],
            dir,
            own: Vec::new(),
        }
    }

    // The files a module path can live in: the crate's root for no path,
    // otherwise the file its last segment names or that folder's mod.rs.
    pub fn files(&self, path: &[String]) -> Vec<PathBuf> {
        if path.is_empty() {
            return self.roots.clone();
        }
        let folder = path
            .iter()
            .fold(self.dir.clone(), |at, segment| at.join(segment));
        vec![folder.with_extension("rs"), folder.join("mod.rs")]
    }
}

// A file's module path from the folder its crate finds modules in: a mod.rs
// names its folder's module, and a crate's root file names none.
fn module_path(inside: &[&str]) -> Vec<String> {
    match inside {
        [] | ["lib.rs"] | ["main.rs"] => Vec::new(),
        [folders @ .., last] => {
            let mut path: Vec<String> = folders.iter().map(|folder| folder.to_string()).collect();
            if *last != "mod.rs" {
                path.push(last.trim_end_matches(".rs").to_string());
            }
            path
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use super::CrateLayout;

    // Each file by its crate's module folder and its own module path.
    fn read(file: &str) -> (String, String) {
        let layout = CrateLayout::of(Path::new("app"), Path::new(file));
        (
            layout.dir.to_string_lossy().into_owned(),
            layout.own.join("::"),
        )
    }

    fn shown(files: Vec<PathBuf>) -> Vec<String> {
        files
            .iter()
            .map(|file| file.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_file_s_crate_and_module_path_follow_cargo_s_layout() {
        for (file, dir, own) in [
            ("app/src/lib.rs", "app/src", ""),
            ("app/src/core/task.rs", "app/src", "core::task"),
            ("app/src/core/mod.rs", "app/src", "core"),
            ("app/src/bin/tool.rs", "app/src/bin", ""),
            ("app/src/bin/tool/main.rs", "app/src/bin/tool", ""),
            ("app/src/bin/tool/args.rs", "app/src/bin/tool", "args"),
            ("app/tests/runs.rs", "app/tests", ""),
            ("app/tests/common/mod.rs", "app/tests", "common"),
            ("app/build.rs", "app", ""),
        ] {
            assert_eq!(read(file), (dir.to_string(), own.to_string()), "{file}");
        }
        let library = CrateLayout::library(Path::new("app"));
        assert_eq!(shown(library.files(&[])), ["app/src/lib.rs"]);
        assert_eq!(
            shown(library.files(&["core".to_string(), "task".to_string()])),
            ["app/src/core/task.rs", "app/src/core/task/mod.rs"]
        );
    }
}
