use tree_sitter::Node;
use tree_sitter::Tree;

use crate::core::trees;
use crate::packs::rust::language::grammar;
use crate::packs::rust::reading::Alias;

const USES: &str = include_str!("../../../../packs/rust/uses.scm");

pub fn uses(tree: &Tree, source: &str) -> Vec<(String, String, usize, bool)> {
    let mut found: Vec<(String, String, usize, bool)> =
        trees::matches(&grammar(), USES, tree, source)
            .into_iter()
            .filter_map(|captures| {
                let module = captures.iter().find(|capture| {
                    capture.name == "module" || capture.name == "declared_module"
                })?;
                let name = captures.iter().find(|capture| capture.name == "name")?;
                Some((
                    from_root(&module.text),
                    name.text.clone(),
                    name.line,
                    module.name == "declared_module",
                ))
            })
            .collect();
    found.extend(
        macro_paths(tree.root_node(), source)
            .into_iter()
            .map(|(module, name, line)| (module, name, line, false)),
    );
    found
}

// Each name a use declaration binds under another name. Importing a trait as
// _ brings its methods without any name to hide.
pub fn aliases(tree: &Tree, source: &str) -> Vec<Alias> {
    let mut found = Vec::new();
    for captures in trees::matches(&grammar(), USES, tree, source) {
        for capture in captures {
            if capture.name != "alias" {
                continue;
            }
            let alias = capture
                .text
                .rsplit(" as ")
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if alias != "_" {
                found.push(Alias {
                    alias,
                    line: capture.line,
                });
            }
        }
    }
    found
}

// A path from the root namespace, as `::name::item` is, names the crate
// after the colons.
fn from_root(path: &str) -> String {
    path.strip_prefix("::").unwrap_or(path).to_string()
}

// Every path written among a macro's tokens, where the parser builds no path
// node: each level of it, as a path node gives them.
fn macro_paths(node: Node, source: &str) -> Vec<(String, String, usize)> {
    if node.kind() == "token_tree" {
        let mut leaves = Vec::new();
        leaves_of(node, &mut leaves);
        return paths_among(&leaves, source);
    }
    let mut found = Vec::new();
    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            found.extend(macro_paths(child, source));
        }
    }
    found
}

fn leaves_of<'tree>(node: Node<'tree>, leaves: &mut Vec<Node<'tree>>) {
    if node.child_count() == 0 {
        leaves.push(node);
        return;
    }
    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            leaves_of(child, leaves);
        }
    }
}

// Each path in a run of tokens: segments joined by `::`, `$crate` read as
// the crate defining the macro.
fn paths_among(leaves: &[Node], source: &str) -> Vec<(String, String, usize)> {
    let mut found = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut joined = false;
    for leaf in leaves {
        let text = &source[leaf.byte_range()];
        let segment = match leaf.kind() {
            "identifier" | "self" | "super" | "crate" => Some(text),
            "metavariable" if text == "$crate" => Some("crate"),
            _ => None,
        };
        match segment {
            Some(segment) => {
                if joined {
                    found.push((
                        path.join("::"),
                        segment.to_string(),
                        leaf.start_position().row + 1,
                    ));
                } else {
                    path.clear();
                }
                path.push(segment.to_string());
                joined = false;
            }
            None if leaf.kind() == "::" && !path.is_empty() && !joined => joined = true,
            None => {
                path.clear();
                joined = false;
            }
        }
    }
    found
}

// The module each glob import in the file takes every name from, with the
// path of any use list holding it.
pub fn globs(node: Node, source: &str) -> Vec<String> {
    if node.kind() == "use_wildcard" {
        let mut path: Vec<String> = node
            .named_child(0)
            .map(|own| source[own.byte_range()].to_string())
            .into_iter()
            .collect();
        let mut above = node.parent();
        while let Some(list) = above {
            if list.kind() == "scoped_use_list" {
                if let Some(prefix) = list.child_by_field_name("path") {
                    path.insert(0, source[prefix.byte_range()].to_string());
                }
            }
            above = list.parent();
        }
        if path.is_empty() {
            return Vec::new();
        }
        return vec![from_root(&path.join("::"))];
    }
    let mut found = Vec::new();
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            found.extend(globs(child, source));
        }
    }
    found
}
