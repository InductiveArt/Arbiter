use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

// The file a module specifier reaches among the repository's files: itself,
// with a TypeScript extension, or the index of the folder it names.
pub fn existing(path: &Path, present: &HashSet<&PathBuf>) -> Option<PathBuf> {
    let shown = path.to_string_lossy();
    for candidate in [
        shown.to_string(),
        format!("{shown}.ts"),
        format!("{shown}.tsx"),
        format!("{shown}/index.ts"),
        format!("{shown}/index.tsx"),
    ] {
        let candidate = PathBuf::from(candidate);
        if present.contains(&candidate) {
            return Some(candidate);
        }
    }
    None
}

// A path with its dots folded away and each parent climbed.
pub fn normalized(path: &Path) -> PathBuf {
    let mut parts: Vec<&OsStr> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(part) => parts.push(part),
            _ => {}
        }
    }
    parts.iter().collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use super::normalized;

    #[test]
    fn dots_fold_away_and_parents_climb() {
        assert_eq!(
            normalized(Path::new("web/src/./sib")),
            PathBuf::from("web/src/sib")
        );
        assert_eq!(
            normalized(Path::new("web/src/../lib/x")),
            PathBuf::from("web/lib/x")
        );
    }
}
