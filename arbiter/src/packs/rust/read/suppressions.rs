use std::path::Path;

use tree_sitter::Tree;

use crate::core::pack::Suppression;
use crate::core::trees;
use crate::packs::rust::language::grammar;

const SUPPRESSIONS: &str = include_str!("../../../../packs/rust/suppressions.scm");

// Every form Rust quiets a rule in: an allow or expect attribute, outer for
// one item and inner for the whole module around it, an ignored test, and
// unsafe code.
pub fn suppressions(file: &Path, tree: &Tree, source: &str) -> Vec<Suppression> {
    let mut found = Vec::new();
    for captures in trees::matches(&grammar(), SUPPRESSIONS, tree, source) {
        for capture in captures {
            let first = capture.text.lines().next().unwrap_or("").trim().to_string();
            let quiets = first.contains("allow(") || first.contains("expect(");
            let recorded = match capture.name.as_str() {
                "outer" if first.contains("#[ignore") => Some(("#[ignore]".to_string(), false)),
                "outer" if quiets => Some((first, false)),
                "inner" if quiets => Some((first, true)),
                "unsafe" => Some(("unsafe".to_string(), false)),
                "modifiers" if capture.text.contains("unsafe") => {
                    Some(("unsafe fn".to_string(), false))
                }
                _ => None,
            };
            if let Some((text, blanket)) = recorded {
                found.push(Suppression {
                    file: file.to_path_buf(),
                    line: capture.line,
                    text,
                    blanket,
                });
            }
        }
    }
    found
}
