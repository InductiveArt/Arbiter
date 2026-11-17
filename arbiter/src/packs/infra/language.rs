use crate::core::stance::Stance::Held;
use crate::core::stance::Stance::Never;
use crate::core::stance::Stance::Open;
use crate::core::stance::Stances;

// The stance deployment manifests take on every rule a pack answers: held,
// open, or never with the reason a manifest cannot break it.
pub static STANCES: Stances = Stances {
    reexport: Never("a manifest declares state; it imports nothing"),
    self_entry: Never("a manifest declares state; it imports nothing"),
    interior_import: Never("a manifest declares state; it imports nothing"),
    hidden_name: Never("a manifest declares state; it names no symbols"),
    crowded_import: Never("a manifest declares state; it imports nothing"),
    phantom_import: Never("a manifest declares state; it imports nothing"),
    comment_overreach: Held,
    undeclared_dependency: Never("a manifest's third parties are images, pinned by digest"),
    unused_dependency: Never("a manifest's third parties are images, pinned by digest"),
    undeclared_protocol: Never("a manifest declares state; it names no classes"),
    unpinned_image: Held,
    tautology: Never("a manifest declares state; it asserts nothing"),
    stale_suppression: Never("a manifest's third parties are images, pinned by digest"),
    unset_dial: Open,
    lowered_dial: Open,
    untested_branch: Never("a manifest declares state; nothing of it executes"),
    off_channel_dependency: Open,
    stale_generated: Never(
        "the proto pack checks every unit's buf.gen.yaml, whatever its language",
    ),
    inter_language_break: Never(
        "the proto pack checks every unit's buf.gen.yaml, whatever its language",
    ),
};

pub const LABEL: &str = "infra";

pub fn handles(name: &str) -> bool {
    name.ends_with(".yaml") || name.ends_with(".yml")
}
