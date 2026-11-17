// assert!(true) and assert_eq! on a value paired with itself: always true,
// asserting nothing.
pub fn tautologies(node: tree_sitter::Node, source: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.kind() == "macro_invocation" {
            let name = node
                .child_by_field_name("macro")
                .and_then(|name| name.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            let tokens = node
                .named_child(node.named_child_count().saturating_sub(1))
                .filter(|last| last.kind() == "token_tree")
                .and_then(|last| last.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            let inner = tokens.trim_start_matches('(').trim_end_matches(')').trim();
            if name == "assert" && inner == "true" {
                found.push(("asserts true".to_string(), node.start_position().row + 1));
            }
            if name == "assert_eq" {
                if let Some((left, right)) = paired(inner) {
                    if left == right && !left.is_empty() {
                        found.push((
                            format!("asserts {left} equals itself"),
                            node.start_position().row + 1,
                        ));
                    }
                }
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

// The two macro arguments split at the one comma outside any nesting.
fn paired(tokens: &str) -> Option<(String, String)> {
    let mut depth = 0;
    let mut split = None;
    for (at, part) in tokens.char_indices() {
        match part {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                if split.is_some() {
                    return None;
                }
                split = Some(at);
            }
            _ => {}
        }
    }
    let at = split?;
    Some((
        tokens[..at].trim().to_string(),
        tokens[at + 1..].trim().to_string(),
    ))
}
