use std::path::Path;

use crate::core::repository::Repo;
use crate::core::trees;
use crate::packs::typescript::import_shape::import_shapes;
use crate::packs::typescript::import_shape::ImportShape;
use crate::packs::typescript::language::generated;
use crate::packs::typescript::language::grammar;
use crate::packs::typescript::language::handles;

// Repairs what the pack can repair by itself: each crowded import split to
// one import per line, in every file that is not a fixture's or a generator's.
pub fn mend(root: &Path, repo: &Repo) -> Vec<String> {
    let mut notes = Vec::new();
    for file in &repo.files {
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if !handles(&name) || generated(&name) {
            continue;
        }
        // A unit on a test path is a fixture: data for some test, never repaired.
        let unit = repo.unit_of(file);
        let home = repo.units[unit].path.clone();
        if repo.is_test(unit, &home) {
            continue;
        }
        let path = root.join(file);
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(tree) = trees::parse(&grammar(), &source) else {
            continue;
        };
        let shapes = import_shapes(&tree, &source);
        if let Some(mended) = split_imports(&source, &shapes) {
            if std::fs::write(&path, mended).is_ok() {
                notes.push(format!("{} split to one import per line", file.display()));
            }
        }
    }
    notes
}

// Splits each crowded statement where it stands, never reordering: the order
// of side effects is the file's own business.
fn split_imports(source: &str, shapes: &[ImportShape]) -> Option<String> {
    let mut text = source.to_string();
    let mut changed = false;
    for shape in shapes.iter().rev() {
        let names = usize::from(shape.default.is_some())
            + usize::from(shape.namespace.is_some())
            + shape.named.len();
        if names <= 1 {
            continue;
        }
        let head = if shape.type_only {
            "import type"
        } else {
            "import"
        };
        let mut lines = Vec::new();
        if let Some(default) = &shape.default {
            lines.push(format!("{head} {default} from \"{}\";", shape.module));
        }
        if let Some(namespace) = &shape.namespace {
            lines.push(format!("{head} {namespace} from \"{}\";", shape.module));
        }
        for written in &shape.named {
            let (form, spec) = match written.strip_prefix("type ") {
                Some(rest) if !shape.type_only => ("import type", rest),
                _ => (head, written.as_str()),
            };
            lines.push(format!("{form} {{ {spec} }} from \"{}\";", shape.module));
        }
        text.replace_range(shape.start..shape.end, &lines.join("\n"));
        changed = true;
    }
    changed.then_some(text)
}
