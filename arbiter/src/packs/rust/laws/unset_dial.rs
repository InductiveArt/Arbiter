use std::path::Path;

use crate::domain::finding::Finding;
use crate::packs::rust::language::MANIFEST;

// A unit with no manifest declares no environment at all.
pub fn missing(home: &Path) -> Finding {
    Finding::dial(
        home,
        MANIFEST,
        "unset-dial",
        "no Cargo.toml; the unit declares no environment".to_string(),
    )
}

// A manifest that does not parse leaves every dial unread.
pub fn unparsable(home: &Path) -> Finding {
    Finding::dial(
        home,
        MANIFEST,
        "unset-dial",
        "Cargo.toml does not parse; every dial is unreadable".to_string(),
    )
}
