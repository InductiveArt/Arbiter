// The whole grammar: each verb with its one line. The bare call prints it,
// and a claim holds the skill to these exact sentences.
pub const VERBS: &[(&str, &str)] = &[
    (
        "gate [--fix] [--cold] [--no-brief]",
        "judge everything; red names what stands between the code and clean",
    ),
    (
        "find [rule]",
        "every tool-free finding, as JSON lines, in seconds; a rule names one law's instances",
    ),
    (
        "find fallout <name>",
        "every definition of a name and each resolved use",
    ),
    (
        "find deps <unit>",
        "what one unit stands on, declared against resolved",
    ),
    (
        "find ledger",
        "every suppression, counted against the reference",
    ),
    (
        "find rhymes",
        "structurally equal functions, largest first; a measure, never findings",
    ),
    (
        "brief [case]",
        "every need's state against the claims tree; a case is one slice",
    ),
    (
        "rules [rule]",
        "the matrix and each law; one rule prints its whys",
    ),
    (
        "accept",
        "record suppressions, dependencies and claims: the user's signature",
    ),
    (
        "adopt",
        "install the agent layer; arbiter-owned files refresh with the binary",
    ),
];

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// An adopted file's last line names its writer, in the comment form the
// file's own syntax reads.
pub fn stamped(place: &str, body: &str) -> String {
    let mark = if place.ends_with(".sh") {
        format!("# arbiter {VERSION}")
    } else {
        format!("<!-- arbiter {VERSION} -->")
    };
    format!("{}\n{mark}\n", body.trim_end())
}

pub fn written_by(text: &str) -> Option<(u64, u64, u64)> {
    let line = text.lines().rev().find(|line| !line.trim().is_empty())?;
    let line = line
        .trim()
        .trim_start_matches("<!--")
        .trim_end_matches("-->")
        .trim_start_matches('#')
        .trim();
    triple(line.strip_prefix("arbiter ")?)
}

// Whether a file's stamp names a newer arbiter than this binary: writing
// over it would move the law backwards.
pub fn ahead(text: &str) -> bool {
    match (written_by(text), triple(VERSION)) {
        (Some(theirs), Some(mine)) => theirs > mine,
        _ => false,
    }
}

fn triple(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.');
    let numbers = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    if parts.next().is_some() {
        return None;
    }
    Some(numbers)
}
