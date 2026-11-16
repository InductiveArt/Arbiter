use std::path::Path;
use std::path::PathBuf;

use tree_sitter::Language;

use crate::core::pack::Repo;
use crate::core::stance::Stance::Held;
use crate::core::stance::Stance::Never;
use crate::core::stance::Stance::Open;
use crate::core::stance::Stances;

// The language's stance on every rule a pack answers: held, open, or never
// with the reason Rust cannot break it.
pub static STANCES: Stances = Stances {
    reexport: Held,
    self_entry: Never("a crate reaches its own items as crate::, never through its published name"),
    interior_import: Never("pub already walls a crate's interior"),
    hidden_name: Held,
    crowded_import: Held,
    phantom_import: Open,
    comment_overreach: Held,
    undeclared_dependency: Held,
    unused_dependency: Held,
    undeclared_protocol: Never("a trait is nominal; nothing implements invisibly"),
    unpinned_image: Never("the language names no container images"),
    tautology: Held,
    stale_suppression: Held,
    unset_dial: Held,
    lowered_dial: Held,
    untested_branch: Open,
    off_channel_dependency: Held,
    stale_generated: Never(
        "the proto pack checks every unit's buf.gen.yaml, whatever its language",
    ),
    inter_language_break: Never(
        "the proto pack checks every unit's buf.gen.yaml, whatever its language",
    ),
};

pub const LABEL: &str = "rust";

pub const MANIFEST: &str = "Cargo.toml";

pub const LOCKFILES: &[&str] = &["Cargo.lock"];

pub const OUTPUTS: &[&str] = &["target"];

pub const AXIOMS: &[&str] = &["cargo"];

pub const ADAPTER: &str = include_str!("../../../packs/rust/ecosystem.toml");

// The crates every program reaches without a manifest declaring them.
pub const BUILTIN_CRATES: [&str; 5] = ["std", "core", "alloc", "proc_macro", "test"];

pub fn grammar() -> Language {
    tree_sitter_rust::LANGUAGE.into()
}

pub fn handles(name: &str) -> bool {
    name.ends_with(".rs")
}

// A Cargo.toml declaring a workspace and no package bounds its members.
pub fn container(manifest: &str) -> bool {
    let Ok(parsed) = manifest.parse::<toml::Value>() else {
        return false;
    };
    parsed.get("workspace").is_some() && parsed.get("package").is_none()
}

// Cargo's compatibility: a version's parts up to its first nonzero one
// name its epoch, so 2.0.119 is 2 and 0.14.6 is 0.14.
pub fn epoch(version: &str) -> String {
    let release = version.split(['-', '+']).next().unwrap_or(version);
    let parts: Vec<&str> = release.split('.').collect();
    let kept = parts
        .iter()
        .position(|part| *part != "0")
        .map_or(parts.len(), |at| at + 1);
    parts[..kept].join(".")
}

// Cargo's convention: a crate's binary entry is src/main.rs beside its manifest.
pub fn roots(repo: &Repo) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for manifest in &repo.manifests {
        if manifest.file_name().is_none_or(|name| name != MANIFEST) {
            continue;
        }
        let folder = manifest.parent().unwrap_or(Path::new(""));
        let entry = folder.join("src").join("main.rs");
        if repo.files.contains(&entry) {
            found.push(entry);
        }
    }
    found
}
