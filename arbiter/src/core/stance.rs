// A pack's answer to one rule: held; never, with the reason the language
// cannot break it; or open, work the matrix keeps visible and never a pass.
#[derive(Clone, Copy)]
pub enum Stance {
    Held,
    Never(&'static str),
    Open,
}

// One pack's stance on every rule a pack holds or declines, one field per
// rule. A pack states each field, so a field added for a new rule is a stance
// no pack compiles without.
#[derive(Clone, Copy)]
pub struct Stances {
    pub reexport: Stance,
    pub self_entry: Stance,
    pub interior_import: Stance,
    pub hidden_name: Stance,
    pub crowded_import: Stance,
    pub phantom_import: Stance,
    pub comment_overreach: Stance,
    pub undeclared_dependency: Stance,
    pub unused_dependency: Stance,
    pub undeclared_protocol: Stance,
    pub unpinned_image: Stance,
    pub tautology: Stance,
    pub stale_suppression: Stance,
    pub unset_dial: Stance,
    pub lowered_dial: Stance,
    pub untested_branch: Stance,
    pub off_channel_dependency: Stance,
    pub stale_generated: Stance,
    pub inter_language_break: Stance,
}

impl Stances {
    // The stance on the rule a name gives, when it is one a pack answers. The
    // pattern names every field, so none can go unanswered here either.
    pub fn of(&self, name: &str) -> Option<Stance> {
        let Stances {
            reexport,
            self_entry,
            interior_import,
            hidden_name,
            crowded_import,
            phantom_import,
            comment_overreach,
            undeclared_dependency,
            unused_dependency,
            undeclared_protocol,
            unpinned_image,
            tautology,
            stale_suppression,
            unset_dial,
            lowered_dial,
            untested_branch,
            off_channel_dependency,
            stale_generated,
            inter_language_break,
        } = *self;
        match name {
            "reexport" => Some(reexport),
            "self-entry" => Some(self_entry),
            "interior-import" => Some(interior_import),
            "hidden-name" => Some(hidden_name),
            "crowded-import" => Some(crowded_import),
            "phantom-import" => Some(phantom_import),
            "comment-overreach" => Some(comment_overreach),
            "undeclared-dependency" => Some(undeclared_dependency),
            "unused-dependency" => Some(unused_dependency),
            "undeclared-protocol" => Some(undeclared_protocol),
            "unpinned-image" => Some(unpinned_image),
            "tautology" => Some(tautology),
            "stale-suppression" => Some(stale_suppression),
            "unset-dial" => Some(unset_dial),
            "lowered-dial" => Some(lowered_dial),
            "untested-branch" => Some(untested_branch),
            "off-channel-dependency" => Some(off_channel_dependency),
            "stale-generated" => Some(stale_generated),
            "inter-language-break" => Some(inter_language_break),
            _ => None,
        }
    }
}
