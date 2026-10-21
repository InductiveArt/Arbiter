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
