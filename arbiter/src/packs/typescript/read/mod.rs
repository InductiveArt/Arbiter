mod doors;
pub mod imports;
mod suppressions;
mod tautologies;

use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Tree;

use crate::core::pack::Definition;
use crate::core::prose;
use crate::core::trees;
use crate::packs::typescript::import_shape::import_shapes;
use crate::packs::typescript::language::grammar;
use crate::packs::typescript::read::doors::doors;
use crate::packs::typescript::read::imports::aliases;
use crate::packs::typescript::read::imports::computed;
use crate::packs::typescript::read::suppressions::suppressions;
use crate::packs::typescript::read::tautologies::tautologies;
use crate::packs::typescript::reading::CrowdedImport;
use crate::packs::typescript::reading::Import;
use crate::packs::typescript::reading::Overreach;
use crate::packs::typescript::reading::Reading;
use crate::packs::typescript::reading::Reexport;
use crate::packs::typescript::reading::Tautology;

const DEFINITIONS: &str = include_str!("../../../../packs/typescript/definitions.scm");
const MENTIONS: &str = include_str!("../../../../packs/typescript/mentions.scm");
const REEXPORTS: &str = include_str!("../../../../packs/typescript/reexports.scm");
const PROSE: &str = include_str!("../../../../packs/typescript/prose.scm");

// One file read whole: its reading, and the definitions it offers when it is
// a surface. A file the grammar cannot parse leaves nothing.
pub fn read(file: &Path, source: &str, surface: bool) -> Option<(Reading, Vec<Definition>)> {
    let tree = trees::parse(&grammar(), source)?;
    let mut reading = Reading {
        file: file.to_path_buf(),
        ..Reading::default()
    };
    for (from, name, line, typed) in imports::imports(&tree, source) {
        if typed {
            reading.typed.insert(line);
        }
        reading.imports.push(Import { from, name, line });
    }
    reading.suppressions = suppressions(file, &tree, source);
    reading.tautologies = tautologies(tree.root_node(), source)
        .into_iter()
        .map(|(text, line)| Tautology { text, line })
        .collect();
    doors(tree.root_node(), source, &mut reading.doors);
    if !surface {
        return Some((reading, Vec::new()));
    }
    for captures in trees::matches(&grammar(), REEXPORTS, &tree, source) {
        for capture in captures {
            reading.reexports.push(Reexport {
                star: capture.name == "star",
                from: capture.text,
                line: capture.line,
            });
        }
    }
    reading.aliases = aliases(&tree, source);
    reading.computed = computed(&tree, source);
    for shape in import_shapes(&tree, source) {
        let names = usize::from(shape.default.is_some())
            + usize::from(shape.namespace.is_some())
            + shape.named.len();
        if names > 1 {
            reading.crowded.push(CrowdedImport {
                names,
                line: shape.row,
            });
        }
    }
    let mentions = mentions(&tree, source);
    let idents: HashSet<String> = mentions.iter().map(|(name, _)| name.clone()).collect();
    let (comments, quoted) = trees::prose_parts(&grammar(), PROSE, &tree, source);
    for (token, line) in prose::overreaches(&comments, &idents, &quoted) {
        reading.overreaches.push(Overreach { token, line });
    }
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

pub fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&grammar(), DEFINITIONS, tree, source, "def")
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&grammar(), MENTIONS, tree, source, "mention")
}
