use std::path::Path;

use tree_sitter::Language;

use crate::core::stance::Stance::Held;
use crate::core::stance::Stance::Never;
use crate::core::stance::Stance::Open;
use crate::core::stance::Stances;

// The language's stance on every rule a pack answers: held, open, or never
// with the reason C++ cannot break it.
pub static STANCES: Stances = Stances {
    reexport: Open,
    self_entry: Open,
    interior_import: Open,
    hidden_name: Open,
    crowded_import: Never("an include names one header by syntax"),
    phantom_import: Open,
    comment_overreach: Held,
    undeclared_dependency: Open,
    unused_dependency: Open,
    undeclared_protocol: Open,
    unpinned_image: Never("the language names no container images"),
    tautology: Open,
    stale_suppression: Open,
    unset_dial: Open,
    lowered_dial: Open,
    untested_branch: Open,
    off_channel_dependency: Open,
    stale_generated: Open,
    inter_language_break: Open,
};

pub const LABEL: &str = "cpp";

pub fn grammar() -> Language {
    tree_sitter_cpp::LANGUAGE.into()
}

pub fn handles(name: &str) -> bool {
    let Some(extension) = name.rsplit('.').next() else {
        return false;
    };
    matches!(extension, "cc" | "cpp" | "cxx" | "h" | "hh" | "hpp")
}

// A header is where a C++ unit offers its names.
pub fn is_header(file: &Path) -> bool {
    matches!(
        file.extension().and_then(|extension| extension.to_str()),
        Some("h" | "hh" | "hpp")
    )
}
