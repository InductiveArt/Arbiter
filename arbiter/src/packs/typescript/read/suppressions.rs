use std::path::Path;

use tree_sitter::Tree;

use crate::core::suppression::Suppression;
use crate::core::trees;
use crate::packs::typescript::language::grammar;

const SUPPRESSIONS: &str = include_str!("../../../../packs/typescript/suppressions.scm");

// Every form TypeScript quiets a rule in: a comment for the linter or the
// compiler, and a test skipped, focused or postponed.
pub fn suppressions(file: &Path, tree: &Tree, source: &str) -> Vec<Suppression> {
    let mut found = Vec::new();
    for captures in trees::matches(&grammar(), SUPPRESSIONS, tree, source) {
        for capture in captures {
            if let Some(blanket) = suppression_of(&capture.text) {
                found.push(Suppression {
                    file: file.to_path_buf(),
                    line: capture.line,
                    text: capture.text.trim().to_string(),
                    blanket,
                });
            }
        }
    }
    for (text, line) in marked(tree.root_node(), source) {
        found.push(Suppression {
            file: file.to_path_buf(),
            line,
            text,
            blanket: false,
        });
    }
    found
}

// Whether a comment suppresses, and whether it quiets more than one named rule.
fn suppression_of(text: &str) -> Option<bool> {
    if text.contains("biome-ignore") || text.contains("@ts-expect-error") {
        return Some(false);
    }
    if text.contains("@ts-ignore") || text.contains("@ts-nocheck") {
        return Some(true);
    }
    None
}

// A skipped, focused or postponed test is a suppression the ledger counts:
// skip and todo silence the test, only silences every sibling.
fn marked(node: tree_sitter::Node, source: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.kind() == "member_expression" {
            let object = node
                .child_by_field_name("object")
                .filter(|object| object.kind() == "identifier")
                .and_then(|object| object.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            let property = node
                .child_by_field_name("property")
                .and_then(|property| property.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            if matches!(object, "describe" | "it" | "test" | "bench")
                && matches!(property, "skip" | "todo" | "only")
            {
                found.push((
                    format!("{object}.{property}"),
                    node.start_position().row + 1,
                ));
            }
        }
        let mut walker = node.walk();
        for child in node.children(&mut walker) {
            stack.push(child);
        }
    }
    found.sort_by_key(|(_, line)| *line);
    found
}
