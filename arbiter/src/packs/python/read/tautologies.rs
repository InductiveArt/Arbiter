// assert True and assert on a value compared to itself: always true, asserting
// nothing.
pub fn tautologies(node: tree_sitter::Node, source: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.kind() == "assert_statement" {
            if let Some(condition) = node.named_child(0) {
                let text = condition.utf8_text(source.as_bytes()).unwrap_or("");
                if condition.kind() == "true" || condition.kind() == "integer" {
                    found.push((format!("asserts {text}"), node.start_position().row + 1));
                }
                if condition.kind() == "comparison_operator" {
                    let mut walker = condition.walk();
                    let sides: Vec<String> = condition
                        .named_children(&mut walker)
                        .filter(|&side| {
                            matches!(
                                side.kind(),
                                "identifier" | "integer" | "string" | "true" | "false"
                            )
                        })
                        .map(|side| side.utf8_text(source.as_bytes()).unwrap_or("").to_string())
                        .collect();
                    if sides.len() == 2 && sides[0] == sides[1] && text.contains("==") {
                        found.push((
                            format!("asserts {} equals itself", sides[0]),
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
