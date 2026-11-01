use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

pub struct Capture {
    pub name: String,
    pub text: String,
    pub line: usize,
}

pub fn parse(language: &Language, source: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(language).ok()?;
    parser.parse(source, None)
}

// The text and line of every capture bearing one name.
pub fn texts(
    language: &Language,
    query: &str,
    tree: &Tree,
    source: &str,
    wanted: &str,
) -> Vec<(String, usize)> {
    matches(language, query, tree, source)
        .into_iter()
        .flatten()
        .filter(|capture| capture.name == wanted)
        .map(|capture| (capture.text, capture.line))
        .collect()
}

// A file's comments with their lines, and everything it quotes as one text.
pub fn prose_parts(
    language: &Language,
    query: &str,
    tree: &Tree,
    source: &str,
) -> (Vec<(String, usize)>, String) {
    let mut comments = Vec::new();
    let mut quoted = String::new();
    for captures in matches(language, query, tree, source) {
        for capture in captures {
            match capture.name.as_str() {
                "comment" => comments.push((capture.text, capture.line)),
                "quoted" => {
                    quoted.push_str(&capture.text);
                    quoted.push('\n');
                }
                _ => {}
            }
        }
    }
    (comments, quoted)
}

pub fn matches(
    language: &Language,
    query_source: &str,
    tree: &Tree,
    source: &str,
) -> Vec<Vec<Capture>> {
    let query = Query::new(language, query_source).expect("a pack's query parses");
    let mut cursor = QueryCursor::new();
    let mut found = Vec::new();
    let mut iterated = cursor.matches(&query, tree.root_node(), source.as_bytes());
    while let Some(matched) = iterated.next() {
        let captures = matched
            .captures
            .iter()
            .map(|capture| Capture {
                name: query.capture_names()[capture.index as usize].to_string(),
                text: capture
                    .node
                    .utf8_text(source.as_bytes())
                    .unwrap_or("")
                    .to_string(),
                line: capture.node.start_position().row + 1,
            })
            .collect();
        found.push(captures);
    }
    found
}

// One function-grain shape: its skeleton digested with every name erased
// (bones) and digested again with each leaf's text kept (flesh). Equal bones
// under unequal flesh is the same structure in different words.
pub struct Shape {
    pub title: String,
    pub line: usize,
    pub ending: usize,
    pub leaves: usize,
    pub bones: String,
    pub flesh: String,
}

// Below this, shapes repeat by chance; above it, by hand.
const LEAST_LEAVES: usize = 25;

pub fn shapes(language: &Language, source: &str) -> Vec<Shape> {
    let Some(tree) = parse(language, source) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    collected(tree.root_node(), source, &mut found);
    found
}

fn collected(node: tree_sitter::Node, source: &str, found: &mut Vec<Shape>) {
    if node.kind().contains("function") || node.kind().contains("method") {
        let mut bones = crate::core::warm::Digest::new();
        let mut flesh = crate::core::warm::Digest::new();
        let leaves = eaten(node, source, &mut bones, &mut flesh);
        if leaves >= LEAST_LEAVES {
            found.push(Shape {
                title: titled(node, source),
                line: node.start_position().row + 1,
                ending: node.end_position().row + 1,
                leaves,
                bones: bones.hex(),
                flesh: flesh.hex(),
            });
        }
    }
    let mut walker = node.walk();
    for child in node.children(&mut walker) {
        collected(child, source, found);
    }
}

// Every node's kind feeds both digests; a leaf's text feeds only the flesh, so
// a rename moves the flesh and never the bones. Comments are prose, not shape.
fn eaten(
    node: tree_sitter::Node,
    source: &str,
    bones: &mut crate::core::warm::Digest,
    flesh: &mut crate::core::warm::Digest,
) -> usize {
    if node.kind().contains("comment") {
        return 0;
    }
    bones.eat(node.kind().as_bytes());
    bones.eat(&[0]);
    flesh.eat(node.kind().as_bytes());
    flesh.eat(&[0]);
    if node.child_count() == 0 {
        flesh.eat(node.utf8_text(source.as_bytes()).unwrap_or("").as_bytes());
        flesh.eat(&[0]);
        return 1;
    }
    let mut walker = node.walk();
    let mut leaves = 0;
    for child in node.children(&mut walker) {
        leaves += eaten(child, source, bones, flesh);
    }
    leaves
}

// The declared name, or the variable an anonymous function is bound to.
fn titled(node: tree_sitter::Node, source: &str) -> String {
    if let Some(name) = node.child_by_field_name("name") {
        return name.utf8_text(source.as_bytes()).unwrap_or("").to_string();
    }
    if let Some(parent) = node.parent() {
        if parent.kind() == "variable_declarator" {
            if let Some(name) = parent.child_by_field_name("name") {
                return name.utf8_text(source.as_bytes()).unwrap_or("").to_string();
            }
        }
    }
    String::new()
}
