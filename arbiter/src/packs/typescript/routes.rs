use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::pack::Repo;
use crate::packs::typescript::language::MANIFEST;
use crate::packs::typescript::paths::existing;
use crate::packs::typescript::paths::normalized;

pub struct Routes {
    pub names: HashMap<String, PathBuf>,
    // Wildcard routes kept as written, the star standing for one span of a specifier.
    pub patterns: Vec<String>,
    pub aliases: HashMap<usize, HashMap<String, String>>,
    pub packages: HashSet<String>,
    // Every route name a manifest writes, kept even when no repository file answers
    // it, since a route pointing at built output is declared all the same.
    pub declared: HashSet<String>,
    // Every file an exports route names: the declared surface of its unit. An
    // imports alias is inside routing and blesses nothing.
    pub surfaces: HashSet<PathBuf>,
    // The surfaces a manifest wrote a route for, as opposed to the imputed
    // src/index fallback: only a written route is the user's declaration to hold.
    pub written: HashSet<PathBuf>,
}

// Where each workspace package enters and where each unit's subpath aliases lead,
// read from the package.json manifests.
pub fn manifest_routes(root: &Path, repo: &Repo, present: &HashSet<&PathBuf>) -> Routes {
    let mut routes = Routes {
        names: HashMap::new(),
        patterns: Vec::new(),
        aliases: HashMap::new(),
        packages: HashSet::new(),
        declared: HashSet::new(),
        surfaces: HashSet::new(),
        written: HashSet::new(),
    };
    for manifest in &repo.manifests {
        if manifest.file_name().is_none_or(|name| name != MANIFEST) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(manifest)) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let folder = manifest.parent().unwrap_or(Path::new("")).to_path_buf();
        if let Some(name) = parsed.get("name").and_then(serde_json::Value::as_str) {
            routes.packages.insert(name.to_string());
            for (route, path) in exported(name, &parsed) {
                if route.contains('*') {
                    routes.patterns.push(route);
                    continue;
                }
                routes.declared.insert(route.clone());
                // A route whose conditions all point at built output falls back to the
                // src file the output is built from, the way the bare name falls back
                // to src/index.
                let fallback = match route
                    .strip_prefix(name)
                    .and_then(|rest| rest.strip_prefix('/'))
                {
                    Some(sub) => format!("src/{sub}"),
                    None => "src/index".to_string(),
                };
                let entry = existing(&normalized(&folder.join(&path)), present)
                    .or_else(|| existing(&normalized(&folder.join(fallback)), present));
                if let Some(entry) = entry {
                    routes.surfaces.insert(entry.clone());
                    routes.written.insert(entry.clone());
                    routes.names.insert(route, entry);
                }
            }
            if !routes.names.contains_key(name) {
                if let Some(entry) = existing(&normalized(&folder.join("src/index")), present) {
                    routes.surfaces.insert(entry.clone());
                    routes.names.insert(name.to_string(), entry);
                }
            }
        }
        if let Some(imports) = parsed.get("imports").and_then(serde_json::Value::as_object) {
            let unit = repo.unit_of(manifest);
            let map = routes.aliases.entry(unit).or_default();
            for (alias, target) in imports {
                if let Some(path) = target.as_str() {
                    map.insert(alias.clone(), path.to_string());
                }
            }
        }
    }
    routes
}

// Every route the manifest exports with the file it enters, the source condition
// first, since that is the file in the repository.
fn exported(name: &str, parsed: &serde_json::Value) -> Vec<(String, String)> {
    let mut found = Vec::new();
    if let Some(exports) = parsed.get("exports") {
        if let Some(map) = exports.as_object() {
            if map.keys().any(|key| key.starts_with('.')) {
                for (key, value) in map {
                    let Some(path) = condition_source(value) else {
                        continue;
                    };
                    let route = if key == "." {
                        name.to_string()
                    } else {
                        format!("{name}/{}", key.trim_start_matches("./"))
                    };
                    found.push((route, path));
                }
                return found;
            }
        }
        if let Some(path) = condition_source(exports) {
            found.push((name.to_string(), path));
            return found;
        }
    }
    if let Some(main) = parsed.get("main").and_then(serde_json::Value::as_str) {
        found.push((name.to_string(), main.to_string()));
    }
    found
}

fn condition_source(value: &serde_json::Value) -> Option<String> {
    if let Some(path) = value.as_str() {
        return Some(path.to_string());
    }
    for condition in ["source", "import", "default"] {
        if let Some(path) = value.get(condition).and_then(serde_json::Value::as_str) {
            return Some(path.to_string());
        }
    }
    None
}

// Whether a specifier walks a wildcard route, the star standing for one span.
pub fn routed(from: &str, pattern: &str) -> bool {
    match pattern.split_once('*') {
        Some((head, tail)) => {
            from.len() >= head.len() + tail.len() && from.starts_with(head) && from.ends_with(tail)
        }
        None => from == pattern,
    }
}

#[cfg(test)]
mod tests {
    use super::routed;

    #[test]
    fn a_wildcard_route_admits_only_what_it_spans() {
        assert!(routed("@ui/icons/arrow", "@ui/icons/*"));
        assert!(!routed("@ui/parts/arrow", "@ui/icons/*"));
        assert!(!routed("@ui/icons", "@ui/icons/*"));
        assert!(routed("@ui/test", "@ui/test"));
    }
}
