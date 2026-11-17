use tree_sitter::Node;

pub fn declared_modules(node: Node, source: &str) -> Vec<String> {
    let mut found = Vec::new();
    if node.kind() == "mod_item" {
        if let Some(name) = node.child_by_field_name("name") {
            found.push(source[name.byte_range()].to_string());
        }
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            found.extend(declared_modules(child, source));
        }
    }
    found
}

// The file each `include!` of a literal path names, as written.
pub fn includes(node: Node, source: &str) -> Vec<String> {
    if node.kind() == "macro_invocation" {
        let named = node
            .child_by_field_name("macro")
            .is_some_and(|name| &source[name.byte_range()] == "include");
        let literal = (0..node.named_child_count())
            .filter_map(|index| node.named_child(index))
            .find(|child| child.kind() == "token_tree")
            .and_then(|tokens| tokens.named_child(0))
            .filter(|first| first.kind() == "string_literal");
        return match (named, literal) {
            (true, Some(literal)) => {
                vec![source[literal.byte_range()].trim_matches('"').to_string()]
            }
            _ => Vec::new(),
        };
    }
    (0..node.named_child_count())
        .filter_map(|index| node.named_child(index))
        .flat_map(|child| includes(child, source))
        .collect()
}
