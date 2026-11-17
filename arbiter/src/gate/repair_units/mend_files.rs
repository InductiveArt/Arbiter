use task_runner::context::task_context::TaskContext;
use task_runner::record::task_failure::TaskFailure;

use crate::core::units;
use crate::domain::folder::Folder;
use crate::domain::found_units::FoundUnits;

// Each language mends its own findings in place, in the order the languages
// stand; a note names each file rewritten.
pub fn mend_files(
    context: &mut TaskContext,
    folder: &Folder,
    found: &FoundUnits,
) -> Result<Vec<String>, TaskFailure> {
    let languages = crate::packs::all();
    let repo = units::repo(found.clone(), &languages);
    let mut notes = Vec::new();
    for language in &languages {
        context.checkpoint()?;
        notes.extend(language.mend(&folder.path, &repo));
    }
    context.measure("rewritten", notes.len() as f64);
    Ok(notes)
}
