mod modules;
mod paths;
mod suppressions;
mod tautologies;

use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Tree;

use crate::core::pack::marked_generated;
use crate::core::pack::Definition;
use crate::core::prose;
use crate::core::trees;
use crate::packs::rust::language::grammar;
use crate::packs::rust::read::modules::declared_modules;
use crate::packs::rust::read::modules::includes;
use crate::packs::rust::read::paths::aliases;
use crate::packs::rust::read::paths::globs;
use crate::packs::rust::read::paths::uses;
use crate::packs::rust::read::suppressions::suppressions;
use crate::packs::rust::read::tautologies::tautologies;
use crate::packs::rust::reading::CrowdedUse;
use crate::packs::rust::reading::Entry;
use crate::packs::rust::reading::Import;
use crate::packs::rust::reading::Overreach;
use crate::packs::rust::reading::Reading;
use crate::packs::rust::reading::Reexport;
use crate::packs::rust::reading::Tautology;
use crate::packs::rust::reading::Use;
use crate::packs::rust::use_tree::crowded_uses;

const DEFINITIONS: &str = include_str!("../../../../packs/rust/definitions.scm");
const MENTIONS: &str = include_str!("../../../../packs/rust/mentions.scm");
const PROSE: &str = include_str!("../../../../packs/rust/prose.scm");
const REEXPORTS: &str = include_str!("../../../../packs/rust/reexports.scm");

// One file read whole: its reading, and the definitions it offers when it is
// a surface. A file the grammar cannot parse leaves nothing.
pub fn read(file: &Path, source: &str, surface: bool) -> Option<(Reading, Vec<Definition>)> {
    let tree = trees::parse(&grammar(), source)?;
    let mut reading = Reading {
        file: file.to_path_buf(),
        ..Reading::default()
    };
    for (module, name, line, from_use) in uses(&tree, source) {
        if from_use {
            let first = module.split("::").next().unwrap_or(&module).to_string();
            reading.entries.push(Entry { first, line });
            reading.imports.push(Import {
                name: name.clone(),
                from: module.clone(),
            });
        }
        reading.uses.push(Use { module, name, line });
    }
    // A glob import binds every name its module offers, so each name the
    // file mentions is a use of that name in that module.
    let globbed = globs(tree.root_node(), source);
    if !globbed.is_empty() {
        let mut seen = HashSet::new();
        for (name, line) in mentions(&tree, source) {
            if seen.insert(name.clone()) {
                for module in &globbed {
                    reading.uses.push(Use {
                        module: module.clone(),
                        name: name.clone(),
                        line,
                    });
                }
            }
        }
    }
    reading.modules = declared_modules(tree.root_node(), source);
    let folder = file.parent().unwrap_or(Path::new(""));
    reading.includes = includes(tree.root_node(), source)
        .into_iter()
        .map(|included| folder.join(included))
        .collect();
    // What a generator wrote is its generator's: read for what it uses,
    // no finding lands in it, its suppressions never counted.
    if marked_generated(source) {
        reading.generated = true;
        return Some((reading, Vec::new()));
    }
    reading.suppressions = suppressions(file, &tree, source);
    reading.tautologies = tautologies(tree.root_node(), source)
        .into_iter()
        .map(|(text, line)| Tautology { text, line })
        .collect();
    if !surface {
        return Some((reading, Vec::new()));
    }
    for captures in trees::matches(&grammar(), REEXPORTS, &tree, source) {
        for capture in captures {
            reading.reexports.push(Reexport {
                argument: capture.text,
                line: capture.line,
            });
        }
    }
    reading.aliases = aliases(&tree, source);
    reading.crowded = crowded_uses(tree.root_node())
        .into_iter()
        .map(|(line, names)| CrowdedUse { names, line })
        .collect();
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
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&grammar(), MENTIONS, tree, source, "mention")
}
