use std::path::Path;

use tree_sitter::Node;

use crate::core::pack::marked_generated;
use crate::core::pack::Repo;
use crate::core::trees;
use crate::packs::rust::language::grammar;
use crate::packs::rust::language::handles;
use crate::packs::rust::use_tree::flattened;
use crate::packs::rust::use_tree::use_leaves;

// Repairs what the pack can repair by itself: each crowded use split to one
// use per line, in every file of the repository that is not a fixture's or a
// generator's.
pub fn mend(root: &Path, repo: &Repo) -> Vec<String> {
    let mut notes = Vec::new();
    for file in &repo.files {
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if !handles(&name) {
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
        if marked_generated(&source) {
            continue;
        }
        let Some(tree) = trees::parse(&grammar(), &source) else {
            continue;
        };
        if let Some(mended) = split_uses(tree.root_node(), &source) {
            if std::fs::write(&path, mended).is_ok() {
                notes.push(format!("{} split to one use per line", file.display()));
            }
        }
    }
    notes
}

// Splits each crowded use where it stands. An attributed use is left for the
// hand: the attribute would govern only the first line a split writes.
fn split_uses(root: Node, source: &str) -> Option<String> {
    let mut spots: Vec<(usize, usize, String)> = Vec::new();
    collect_splits(root, source, &mut spots);
    if spots.is_empty() {
        return None;
    }
    let mut text = source.to_string();
    for (start, end, replacement) in spots.into_iter().rev() {
        text.replace_range(start..end, &replacement);
    }
    Some(text)
}

fn collect_splits(node: Node, source: &str, spots: &mut Vec<(usize, usize, String)>) {
    if node.kind() == "use_declaration" {
        let mut names = 0;
        let mut visibility = String::new();
        let mut argument = None;
        for index in 0..node.named_child_count() {
            if let Some(child) = node.named_child(index) {
                if child.kind() == "visibility_modifier" {
                    visibility = format!("{} ", &source[child.byte_range()]);
                } else {
                    names += use_leaves(child);
                    argument = Some(child);
                }
            }
        }
        let Some(argument) = argument else {
            return;
        };
        if names <= 1
            || node
                .prev_named_sibling()
                .is_some_and(|previous| previous.kind() == "attribute_item")
        {
            return;
        }
        let start = node.start_byte();
        let line_start = source[..start].rfind('\n').map(|at| at + 1).unwrap_or(0);
        let indent = &source[line_start..start];
        let lines: Vec<String> = flattened("", argument, source)
            .iter()
            .map(|path| format!("{visibility}use {path};"))
            .collect();
        spots.push((start, node.end_byte(), lines.join(&format!("\n{indent}"))));
        return;
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            collect_splits(child, source, spots);
        }
    }
}
