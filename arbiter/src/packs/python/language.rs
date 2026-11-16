use tree_sitter::Language;

use crate::core::stance::Stance::Held;
use crate::core::stance::Stance::Never;
use crate::core::stance::Stance::Open;
use crate::core::stance::Stances;

// The language's stance on every rule a pack answers: held, open, or never
// with the reason Python cannot break it.
pub static STANCES: Stances = Stances {
    reexport: Held,
    self_entry: Never("an __init__ stays empty, so a unit has no barrel of its own to enter"),
    interior_import: Never("a module is imported by its defining path; python declares no routes"),
    hidden_name: Held,
    crowded_import: Held,
    phantom_import: Held,
    comment_overreach: Held,
    undeclared_dependency: Held,
    unused_dependency: Held,
    undeclared_protocol: Held,
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

pub const LABEL: &str = "python";

pub const MANIFEST: &str = "pyproject.toml";

pub const LOCKFILES: &[&str] = &["uv.lock"];

pub const OUTPUTS: &[&str] = &["__pycache__"];

pub const AXIOMS: &[&str] = &["uv"];

pub const ADAPTER: &str = include_str!("../../../packs/python/ecosystem.toml");

pub const CONFIGS: &[(&str, &str)] =
    &[("ruff.toml", include_str!("../../../packs/python/ruff.toml"))];

// What the bar refuses in a manifest, and the aliases it allows.
pub const BAR: &str = include_str!("../../../packs/python/bar.toml");

pub fn grammar() -> Language {
    tree_sitter_python::LANGUAGE.into()
}

pub fn handles(name: &str) -> bool {
    name.ends_with(".py")
}

// What protobuf's generators write for Python, by file name.
pub fn generated(name: &str) -> bool {
    name.ends_with("_pb2.py") || name.ends_with("_pb2_grpc.py") || name.ends_with("_pb2.pyi")
}
