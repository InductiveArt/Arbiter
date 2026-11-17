use std::path::Path;

use tree_sitter::Tree;

use crate::core::definition::Definition;
use crate::core::suppression::Suppression;
use crate::core::trees;
use crate::packs::proto::language::grammar;
use crate::packs::proto::reading::Reading;
use crate::packs::proto::reading::TypeRef;

const DEFINITIONS: &str = include_str!("../../../packs/proto/definitions.scm");
const SUPPRESSIONS: &str = include_str!("../../../packs/proto/suppressions.scm");
const USES: &str = include_str!("../../../packs/proto/uses.scm");
const MENTIONS: &str = include_str!("../../../packs/proto/mentions.scm");

// One contract read whole: its reading, and the definitions it offers when it
// is a surface. A contract the grammar cannot parse leaves nothing.
pub fn read(file: &Path, source: &str, surface: bool) -> Option<(Reading, Vec<Definition>)> {
    let tree = trees::parse(&grammar(), source)?;
    let mut reading = Reading {
        file: file.to_path_buf(),
        ..Reading::default()
    };
    for (name, line) in trees::texts(&grammar(), USES, &tree, source, "typeref") {
        reading.typerefs.push(TypeRef { name, line });
    }
    for (path, _) in trees::texts(&grammar(), USES, &tree, source, "import") {
        reading.imports.push(path.trim_matches('"').to_string());
    }
    // A buf:lint:ignore comment quiets the rules it names, or all of them.
    for captures in trees::matches(&grammar(), SUPPRESSIONS, &tree, source) {
        for capture in captures {
            let told = capture.text.trim_start_matches('/').trim().to_string();
            let Some(rest) = told.strip_prefix("buf:lint:ignore") else {
                continue;
            };
            reading.suppressions.push(Suppression {
                file: file.to_path_buf(),
                line: capture.line,
                blanket: rest.trim().is_empty(),
                text: told,
            });
        }
    }
    if !surface {
        return Some((reading, Vec::new()));
    }
    let mentions = mentions(&tree, source);
    let offered = definitions(&tree, source)
        .into_iter()
        .map(|(name, line)| Definition {
            self_used: mentions
                .iter()
                .any(|(mentioned, at)| *mentioned == name && *at != line),
            name,
            line,
        })
        .collect();
    Some((reading, offered))
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&grammar(), DEFINITIONS, tree, source, "def")
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&grammar(), MENTIONS, tree, source, "mention")
}
