use crate::domain::finding::Finding;
use crate::packs::rust::reading::Reading;

pub fn instances(readings: &[Reading]) -> Vec<Finding> {
    readings
        .iter()
        .flat_map(|reading| {
            reading.aliases.iter().map(|alias| {
                Finding::integrity(
                    &reading.file,
                    alias.line,
                    "hidden-name",
                    format!("imported as {}; a symbol keeps its own name", alias.alias),
                )
            })
        })
        .collect()
}
