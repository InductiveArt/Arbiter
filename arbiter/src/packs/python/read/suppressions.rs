use std::path::Path;

use tree_sitter::Tree;

use crate::core::suppression::Suppression;
use crate::core::trees;
use crate::packs::python::language::grammar;

const SUPPRESSIONS: &str = include_str!("../../../../packs/python/suppressions.scm");

// Every form Python quiets a rule in: a comment naming a checker's rule or
// quieting them all, and a test marked to skip or to fail.
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
    if text.contains("noqa") {
        return Some(!text.contains("noqa:"));
    }
    if text.contains("pyright: ignore") {
        return Some(!text.contains("pyright: ignore["));
    }
    if text.contains("type: ignore") {
        return Some(true);
    }
    None
}

// A pytest skip, skipif or xfail is a suppression the ledger counts.
fn marked(node: tree_sitter::Node, source: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.kind() == "decorator" {
            let text = node.utf8_text(source.as_bytes()).unwrap_or("");
            if ["mark.skipif", "mark.skip", "mark.xfail"]
                .iter()
                .any(|form| text.contains(form))
            {
                let first = text.lines().next().unwrap_or("").trim().to_string();
                found.push((first, node.start_position().row + 1));
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
