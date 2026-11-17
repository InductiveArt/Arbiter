use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::trees;
use crate::packs::typescript::language::grammar;
use crate::packs::typescript::read::definitions;

// What each surface file defines of its own, a named re-export counting as
// nothing: an empty answer marks the surface pure.
pub fn surface_definitions(
    root: &Path,
    surfaces: &HashSet<PathBuf>,
) -> HashMap<PathBuf, Vec<(String, usize)>> {
    let mut found = HashMap::new();
    for file in surfaces {
        let Ok(source) = std::fs::read_to_string(root.join(file)) else {
            continue;
        };
        let Some(tree) = trees::parse(&grammar(), &source) else {
            continue;
        };
        let owned = definitions(&tree, &source);
        if !owned.is_empty() {
            found.insert(file.clone(), owned);
        }
    }
    found
}
