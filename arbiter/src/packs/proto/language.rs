use tree_sitter::Language;

use crate::core::stance::Stance::Held;
use crate::core::stance::Stance::Never;
use crate::core::stance::Stance::Open;
use crate::core::stance::Stances;

// The language's stance on every rule a pack answers: held, open, or never
// with the reason a contract cannot break it.
pub static STANCES: Stances = Stances {
    reexport: Never("proto has no re-export form"),
    self_entry: Never("a contract declares no routes"),
    interior_import: Never("a contract declares no routes"),
    hidden_name: Never("proto has no alias form"),
    crowded_import: Never("an import names one file by syntax"),
    phantom_import: Never("the contract is the source others generate from"),
    comment_overreach: Held,
    undeclared_dependency: Held,
    unused_dependency: Held,
    undeclared_protocol: Never("a contract declares shapes, never classes"),
    unpinned_image: Never("the language names no container images"),
    tautology: Never("nothing of a contract executes"),
    stale_suppression: Held,
    unset_dial: Open,
    lowered_dial: Open,
    untested_branch: Never("a contract declares shapes; nothing of it executes"),
    off_channel_dependency: Open,
    stale_generated: Held,
    inter_language_break: Held,
};

pub const LABEL: &str = "proto";

pub const MANIFEST: &str = "buf.yaml";

pub const AXIOMS: &[&str] = &["npx"];

pub const ADAPTER: &str = include_str!("../../../packs/proto/ecosystem.toml");

pub fn grammar() -> Language {
    tree_sitter_proto::LANGUAGE.into()
}

pub fn handles(name: &str) -> bool {
    name.ends_with(".proto")
}
