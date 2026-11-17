// expect(a).toBe(a) with both sides one literal or one name: always true,
// asserting nothing. NaN is the lone exception and no test leans on it.
pub fn tautologies(node: tree_sitter::Node, source: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.kind() == "call_expression" {
            if let Some((left, right)) = asserted_pair(node, source) {
                if left == right {
                    found.push((left, node.start_position().row + 1));
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

fn asserted_pair(call: tree_sitter::Node, source: &str) -> Option<(String, String)> {
    let callee = call.child_by_field_name("function")?;
    if callee.kind() != "member_expression" {
        return None;
    }
    let property = callee
        .child_by_field_name("property")?
        .utf8_text(source.as_bytes())
        .ok()?;
    if !matches!(property, "toBe" | "toEqual" | "toStrictEqual") {
        return None;
    }
    let object = callee.child_by_field_name("object")?;
    if object.kind() != "call_expression"
        || object
            .child_by_field_name("function")?
            .utf8_text(source.as_bytes())
            .ok()?
            != "expect"
    {
        return None;
    }
    let expected = simple_argument(object, source)?;
    let compared = simple_argument(call, source)?;
    Some((expected, compared))
}

// The sole argument when it is a bare literal or name; anything built is not read.
fn simple_argument(call: tree_sitter::Node, source: &str) -> Option<String> {
    let arguments = call.child_by_field_name("arguments")?;
    let mut walker = arguments.walk();
    let named: Vec<tree_sitter::Node> = arguments.named_children(&mut walker).collect();
    let [argument] = named[..] else {
        return None;
    };
    if !matches!(
        argument.kind(),
        "identifier" | "number" | "string" | "true" | "false"
    ) {
        return None;
    }
    Some(argument.utf8_text(source.as_bytes()).ok()?.to_string())
}
