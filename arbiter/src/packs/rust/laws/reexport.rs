use crate::domain::finding::Finding;
use crate::packs::rust::reading::Reading;

pub fn instances(readings: &[Reading]) -> Vec<Finding> {
    readings
        .iter()
        .flat_map(|reading| {
            reading.reexports.iter().map(|reexport| {
                Finding::integrity(
                    &reading.file,
                    reexport.line,
                    "reexport",
                    format!(
                        "re-exports {}; a module offers only its own names",
                        reexport.argument
                    ),
                )
            })
        })
        .collect()
}
