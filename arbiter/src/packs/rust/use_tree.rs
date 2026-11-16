use tree_sitter::Node;

// Counts the leaf names each use binds: more than one on a line hides a
// symbol from a whole-word search pairing it with its module.
pub fn crowded_uses(node: Node) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    if node.kind() == "use_declaration" {
        let mut names = 0;
        for index in 0..node.named_child_count() {
            if let Some(child) = node.named_child(index) {
                if child.kind() != "visibility_modifier" {
                    names += use_leaves(child);
                }
            }
        }
        if names > 1 {
            found.push((node.start_position().row + 1, names));
        }
        return found;
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            found.extend(crowded_uses(child));
        }
    }
    found
}

pub fn use_leaves(node: Node) -> usize {
    match node.kind() {
        "use_list" => {
            let mut sum = 0;
            for index in 0..node.named_child_count() {
                if let Some(child) = node.named_child(index) {
                    sum += use_leaves(child);
                }
            }
            sum
        }
        "scoped_use_list" => node
            .child_by_field_name("list")
            .map(use_leaves)
            .unwrap_or(0),
        _ => 1,
    }
}

pub fn flattened(prefix: &str, node: Node, source: &str) -> Vec<String> {
    let glue = |tail: &str| {
        if prefix.is_empty() {
            tail.to_string()
        } else {
            format!("{prefix}::{tail}")
        }
    };
    match node.kind() {
        "use_list" => {
            let mut all = Vec::new();
            for index in 0..node.named_child_count() {
                if let Some(child) = node.named_child(index) {
                    all.extend(flattened(prefix, child, source));
                }
            }
            all
        }
        "scoped_use_list" => {
            let deeper = node
                .child_by_field_name("path")
                .map(|path| glue(&source[path.byte_range()]))
                .unwrap_or_else(|| prefix.to_string());
            node.child_by_field_name("list")
                .map(|list| flattened(&deeper, list, source))
                .unwrap_or_default()
        }
        "use_as_clause" => {
            let path = node
                .child_by_field_name("path")
                .map(|path| glue(&source[path.byte_range()]))
                .unwrap_or_default();
            let alias = node
                .child_by_field_name("alias")
                .map(|alias| source[alias.byte_range()].to_string())
                .unwrap_or_default();
            vec![format!("{path} as {alias}")]
        }
        "self" => vec![prefix.to_string()],
        _ => vec![glue(&source[node.byte_range()])],
    }
}
