use tree_sitter::Language;

use crate::core::stance::Stance::Held;
use crate::core::stance::Stance::Never;
use crate::core::stance::Stance::Open;
use crate::core::stance::Stances;

// The language's stance on every rule a pack answers: held, open, or never
// with the reason TypeScript cannot break it.
pub static STANCES: Stances = Stances {
    reexport: Held,
    self_entry: Held,
    interior_import: Held,
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

pub const LABEL: &str = "typescript";

pub const MANIFEST: &str = "package.json";

pub const LOCKFILES: &[&str] = &["pnpm-lock.yaml", "package-lock.json"];

pub const OUTPUTS: &[&str] = &["node_modules", "dist"];

pub const AXIOMS: &[&str] = &["node", "npx"];

pub const ADAPTER: &str = include_str!("../../../packs/typescript/ecosystem.toml");

pub const CONFIGS: &[(&str, &str)] = &[(
    "biome.jsonc",
    include_str!("../../../packs/typescript/biome.jsonc"),
)];

// The compiler options the bar demands.
pub const BAR: &str = include_str!("../../../packs/typescript/bar.toml");

pub fn grammar() -> Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

pub fn handles(name: &str) -> bool {
    !name.ends_with(".d.ts") && (name.ends_with(".ts") || name.ends_with(".tsx"))
}

// What protobuf's generators write for TypeScript, by file name.
pub fn generated(name: &str) -> bool {
    name.ends_with("_pb.ts") || name.ends_with(".pb.ts")
}
