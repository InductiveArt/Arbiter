mod doors;
mod imports;
mod suppressions;
mod tautologies;

use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Tree;

use crate::core::pack::Definition;
use crate::core::prose;
use crate::core::trees;
use crate::packs::python::language::grammar;
use crate::packs::python::read::doors::doors;
use crate::packs::python::read::imports::allowed_aliases;
use crate::packs::python::read::imports::captured;
use crate::packs::python::read::imports::imports;
use crate::packs::python::read::imports::loaded;
use crate::packs::python::read::suppressions::suppressions;
use crate::packs::python::read::tautologies::tautologies;
use crate::packs::python::reading::Alias;
use crate::packs::python::reading::Chain;
use crate::packs::python::reading::Computed;
use crate::packs::python::reading::Import;
use crate::packs::python::reading::InitImport;
use crate::packs::python::reading::Overreach;
use crate::packs::python::reading::Plain;
use crate::packs::python::reading::Reading;
use crate::packs::python::reading::Tautology;

const DEFINITIONS: &str = include_str!("../../../../packs/python/definitions.scm");
const MENTIONS: &str = include_str!("../../../../packs/python/mentions.scm");
const PROSE: &str = include_str!("../../../../packs/python/prose.scm");

// One file read whole: its reading, and the definitions it offers when it is
// a surface. A file the grammar cannot parse leaves nothing.
pub fn read(file: &Path, source: &str, surface: bool) -> Option<(Reading, Vec<Definition>)> {
    let tree = trees::parse(&grammar(), source)?;
    let mut reading = Reading {
        file: file.to_path_buf(),
        ..Reading::default()
    };
    let in_init = file.file_name().is_some_and(|name| name == "__init__.py");
    for (module, name, line) in imports(&tree, source) {
        if surface && in_init {
            reading.init_imports.push(InitImport {
                name: name.clone(),
                line,
            });
        }
        reading.imports.push(Import { module, name, line });
    }
    // A star import binds every name its module offers: the module is
    // reached, and each name the file mentions is taken from it.
    let starred = captured(&tree, source, "star");
    if !starred.is_empty() {
        let mut seen = HashSet::new();
        let named: Vec<(String, usize)> = mentions(&tree, source)
            .into_iter()
            .filter(|(name, _)| seen.insert(name.clone()))
            .collect();
        for (module, line) in starred {
            for (name, at) in &named {
                reading.imports.push(Import {
                    module: module.clone(),
                    name: name.clone(),
                    line: *at,
                });
            }
            reading.imports.push(Import {
                module,
                name: "*".to_string(),
                line,
            });
        }
    }
    for (module, line) in captured(&tree, source, "plain") {
        reading.plains.push(Plain { module, line });
    }
    for (module, line) in loaded(&tree, source) {
        match module {
            Some(module) => reading.plains.push(Plain { module, line }),
            None if surface => reading.computed.push(Computed { line }),
            None => {}
        }
    }
    for (chain, line) in captured(&tree, source, "chain") {
        reading.chains.push(Chain { chain, line });
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
    let allowed = allowed_aliases();
    for (text, line) in captured(&tree, source, "alias") {
        let alias = text.rsplit(" as ").next().unwrap_or("").trim().to_string();
        if !allowed.contains(&alias) {
            reading.aliases.push(Alias { alias, line });
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

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&grammar(), DEFINITIONS, tree, source, "def")
        .into_iter()
        .filter(|(name, _)| !name.starts_with('_'))
        .collect()
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&grammar(), MENTIONS, tree, source, "mention")
}
