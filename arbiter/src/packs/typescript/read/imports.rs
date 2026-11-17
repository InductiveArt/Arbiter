use tree_sitter::Tree;

use crate::core::trees;
use crate::packs::typescript::language::grammar;
use crate::packs::typescript::reading::Alias;
use crate::packs::typescript::reading::Computed;

const USES: &str = include_str!("../../../../packs/typescript/uses.scm");

pub fn imports(tree: &Tree, source: &str) -> Vec<(String, String, usize, bool)> {
    trees::matches(&grammar(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            // A dynamic literal import and a namespace import take names only
            // run time resolves: the star stands for every name the module offers.
            if let Some(dynamic) = captures.iter().find(|capture| capture.name == "dynamic") {
                return Some((dynamic.text.clone(), "*".to_string(), dynamic.line, false));
            }
            let typed = captures.iter().any(|capture| capture.name == "typemark");
            if captures.iter().any(|capture| capture.name == "namespace") {
                let from = captures.iter().find(|capture| capture.name == "source")?;
                return Some((from.text.clone(), "*".to_string(), from.line, typed));
            }
            // A default import binds no exported name, and "default" is a word
            // no export can carry, so it resolves to nothing by construction.
            if captures.iter().any(|capture| capture.name == "default") {
                let from = captures.iter().find(|capture| capture.name == "source")?;
                return Some((from.text.clone(), "default".to_string(), from.line, typed));
            }
            // A bare import binds nothing at all; the empty name matches no definition.
            if let Some(bare) = captures.iter().find(|capture| capture.name == "bare") {
                return Some((bare.text.clone(), String::new(), bare.line, false));
            }
            let from = captures.iter().find(|capture| capture.name == "source")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((from.text.clone(), name.text.clone(), name.line, typed))
        })
        .collect()
}

// Each name an import binds under another name.
pub fn aliases(tree: &Tree, source: &str) -> Vec<Alias> {
    let mut found = Vec::new();
    for captures in trees::matches(&grammar(), USES, tree, source) {
        for capture in captures {
            if capture.name == "aliased" {
                let alias = capture
                    .text
                    .rsplit(" as ")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                found.push(Alias {
                    alias,
                    line: capture.line,
                });
            }
        }
    }
    found
}

// Each import of a module through a value only run time resolves.
pub fn computed(tree: &Tree, source: &str) -> Vec<Computed> {
    let mut found = Vec::new();
    for captures in trees::matches(&grammar(), USES, tree, source) {
        for capture in captures {
            if capture.name == "computed" {
                found.push(Computed { line: capture.line });
            }
        }
    }
    found
}
