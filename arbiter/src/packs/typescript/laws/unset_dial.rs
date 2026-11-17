use std::path::Path;

use crate::domain::finding::Finding;

// A unit where no tsconfig.json resolves declares no environment at all.
pub fn missing(home: &Path) -> Finding {
    Finding::dial(
        home,
        "tsconfig.json",
        "unset-dial",
        "no tsconfig.json resolves here; the unit declares no environment".to_string(),
    )
}
