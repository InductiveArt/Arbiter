use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use crate::packs::rust::layout::CrateLayout;
use crate::packs::rust::reading::Import;

// The files that can define what a path's module names, read from its first
// segment as the compiler reads it: a name the file imports stands for the
// path it imports, `crate` is the file's own crate, a crate of the
// repository is that crate's library, `self` and `super` are the file's
// module and the ones above it, and any other name is a module under the
// file's own. A path naming `self` or `super` alone stays inside its file.
pub fn module_files(
    file: &Path,
    module: &str,
    unit: &Path,
    imports: &[Import],
    crates: &HashMap<String, PathBuf>,
) -> Vec<PathBuf> {
    if module == "self" || module == "super" {
        return Vec::new();
    }
    let segments = expanded(module, imports);
    let Some((first, rest)) = segments.split_first() else {
        return Vec::new();
    };
    let layout = CrateLayout::of(unit, file);
    let below = |kept: usize, tail: &[String]| -> Vec<String> {
        layout.own[..kept].iter().chain(tail).cloned().collect()
    };
    let paths = match first.as_str() {
        "crate" => vec![rest.to_vec()],
        "self" => vec![below(layout.own.len(), rest)],
        "super" => {
            let ups = segments
                .iter()
                .take_while(|segment| *segment == "super")
                .count();
            // Above the file's module, or above an inline module inside it.
            [ups, ups - 1]
                .iter()
                .filter_map(|up| layout.own.len().checked_sub(*up))
                .map(|kept| below(kept, &segments[ups..]))
                .collect()
        }
        name => match crates.get(&name.replace('_', "-")) {
            Some(home) => return CrateLayout::library(home).files(rest),
            None => vec![below(layout.own.len(), &segments)],
        },
    };
    paths.iter().flat_map(|path| layout.files(path)).collect()
}

// A module's segments with the first read through the file's imports.
fn expanded(module: &str, imports: &[Import]) -> Vec<String> {
    let mut segments: Vec<String> = module.split("::").map(str::to_string).collect();
    if let Some(import) = imports
        .iter()
        .find(|import| import.name == segments[0] && binds(&import.name, &import.from))
    {
        let mut whole: Vec<String> = import.from.split("::").map(str::to_string).collect();
        whole.push(import.name.clone());
        whole.extend(segments.drain(1..));
        segments = whole;
    }
    segments
}

// Whether a name a use declaration binds stands for the path it binds: not
// when that path starts with the name itself, as `use log::log;` does, where
// the first segment is the crate and never the binding.
pub fn binds(name: &str, from: &str) -> bool {
    from.split("::").next() != Some(name)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;
    use std::path::PathBuf;

    use super::module_files;
    use crate::packs::rust::reading::Import;

    fn shown(files: Vec<PathBuf>) -> Vec<String> {
        files
            .iter()
            .map(|file| file.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_path_is_read_from_its_first_segment_as_the_compiler_reads_it() {
        let crates = HashMap::from([("runner".to_string(), PathBuf::from("runner"))]);
        let imports = [Import {
            name: "trees".to_string(),
            from: "crate::core".to_string(),
        }];
        let files = |module: &str| {
            shown(module_files(
                Path::new("app/src/tasks/tools.rs"),
                module,
                Path::new("app"),
                &imports,
                &crates,
            ))
        };
        assert_eq!(
            files("crate::core::task"),
            ["app/src/core/task.rs", "app/src/core/task/mod.rs"]
        );
        assert_eq!(
            files("runner::declaration::task"),
            [
                "runner/src/declaration/task.rs",
                "runner/src/declaration/task/mod.rs"
            ]
        );
        assert_eq!(
            files("trees"),
            ["app/src/core/trees.rs", "app/src/core/trees/mod.rs"],
            "an imported name stands for the path it was imported from"
        );
        assert_eq!(
            files("self::args"),
            [
                "app/src/tasks/tools/args.rs",
                "app/src/tasks/tools/args/mod.rs"
            ]
        );
        assert_eq!(
            files("super::run"),
            [
                "app/src/tasks/run.rs",
                "app/src/tasks/run/mod.rs",
                "app/src/tasks/tools/run.rs",
                "app/src/tasks/tools/run/mod.rs"
            ],
            "super is above the file, or above an inline module inside it"
        );
        assert_eq!(
            files("super::super::gate"),
            [
                "app/src/gate.rs",
                "app/src/gate/mod.rs",
                "app/src/tasks/gate.rs",
                "app/src/tasks/gate/mod.rs"
            ]
        );
        assert_eq!(
            files("args"),
            [
                "app/src/tasks/tools/args.rs",
                "app/src/tasks/tools/args/mod.rs"
            ],
            "any other name is a module under the file's own"
        );
        assert!(files("super").is_empty(), "super alone stays in its file");
    }
}
