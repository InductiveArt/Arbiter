use std::path::Path;

use crate::core::pack::dial;
use crate::domain::finding::Finding;
use crate::packs::python::language::MANIFEST;

// A unit with no manifest declares no environment at all.
pub fn missing(home: &Path) -> Finding {
    dial(
        home,
        MANIFEST,
        "unset-dial",
        "no pyproject.toml; the unit declares no environment".to_string(),
    )
}

// A manifest that does not parse leaves every dial unread.
pub fn unparsable(home: &Path) -> Finding {
    dial(
        home,
        MANIFEST,
        "unset-dial",
        "pyproject.toml does not parse; every dial is unreadable".to_string(),
    )
}
