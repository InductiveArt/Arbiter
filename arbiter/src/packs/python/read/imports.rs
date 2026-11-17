use tree_sitter::Tree;

use crate::core::trees;
use crate::packs::python::language::grammar;
use crate::packs::python::language::BAR;

const USES: &str = include_str!("../../../../packs/python/uses.scm");

pub fn captured(tree: &Tree, source: &str, wanted: &str) -> Vec<(String, usize)> {
    trees::texts(&grammar(), USES, tree, source, wanted)
}

pub fn imports(tree: &Tree, source: &str) -> Vec<(String, String, usize)> {
    trees::matches(&grammar(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            let module = captures.iter().find(|capture| capture.name == "module")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((module.text.clone(), name.text.clone(), name.line))
        })
        .collect()
}

// Run-time imports: Some(module) when the call names a literal, None when
// the value only run time resolves.
pub fn loaded(tree: &Tree, source: &str) -> Vec<(Option<String>, usize)> {
    trees::matches(&grammar(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            let loader = captures.iter().find(|capture| capture.name == "loader")?;
            let importer = matches!(
                loader.text.as_str(),
                "__import__" | "import_module" | "importlib.import_module"
            );
            if !importer {
                return None;
            }
            if let Some(dynamic) = captures.iter().find(|capture| capture.name == "dynamic") {
                return Some((literal_module(&dynamic.text), dynamic.line));
            }
            captures
                .iter()
                .find(|capture| capture.name == "computed")
                .map(|computed| (None, computed.line))
        })
        .collect()
}

// A plain or raw literal names its module; an f-string resolves at run time.
fn literal_module(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let at = trimmed.find(['"', '\''])?;
    let (prefix, quoted) = trimmed.split_at(at);
    if prefix.to_ascii_lowercase().contains('f') {
        return None;
    }
    let inner = quoted.trim_matches(['"', '\'']);
    (!inner.is_empty()).then(|| inner.to_string())
}

pub fn allowed_aliases() -> Vec<String> {
    let Ok(bar) = BAR.parse::<toml::Value>() else {
        return Vec::new();
    };
    bar.get("aliases")
        .and_then(|aliases| aliases.get("allowed"))
        .and_then(toml::Value::as_array)
        .map(|allowed| {
            allowed
                .iter()
                .filter_map(|name| name.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}
