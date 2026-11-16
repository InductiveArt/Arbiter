use std::path::Path;

use tree_sitter::Tree;

use crate::core::pack::Definition;
use crate::core::trees;
use crate::packs::cpp::language::grammar;
use crate::packs::cpp::language::is_header;
use crate::packs::cpp::reading::Call;
use crate::packs::cpp::reading::Reading;

const DEFINITIONS: &str = include_str!("../../../packs/cpp/definitions.scm");
const USES: &str = include_str!("../../../packs/cpp/uses.scm");

// One file read whole: its reading, and the definitions a header offers when
// it is a surface. A file the grammar cannot parse leaves nothing.
pub fn read(file: &Path, source: &str, surface: bool) -> Option<(Reading, Vec<Definition>)> {
    let tree = trees::parse(&grammar(), source)?;
    let mut reading = Reading {
        file: file.to_path_buf(),
        includes: Vec::new(),
        calls: Vec::new(),
    };
    for captures in trees::matches(&grammar(), USES, &tree, source) {
        for capture in captures {
            match capture.name.as_str() {
                "include" => reading
                    .includes
                    .push(capture.text.trim_matches('"').to_string()),
                "use" => reading.calls.push(Call {
                    name: capture.text,
                    line: capture.line,
                }),
                _ => {}
            }
        }
    }
    if !surface || !is_header(file) {
        return Some((reading, Vec::new()));
    }
    let offered = definitions(&tree, source)
        .into_iter()
        .map(|(name, line)| Definition {
            name,
            line,
            self_used: false,
        })
        .collect();
    Some((reading, offered))
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&grammar(), DEFINITIONS, tree, source, "def")
}
