use crate::domain::finding::Finding;
use crate::packs::rust::reading::Reading;

pub fn instances(readings: &[Reading]) -> Vec<Finding> {
    readings
        .iter()
        .flat_map(|reading| {
            reading.crowded.iter().map(|crowded| {
                Finding::integrity(
                    &reading.file,
                    crowded.line,
                    "crowded-import",
                    format!(
                        "names {} symbols in one import; \
                         an import line names one symbol and the module it comes from",
                        crowded.names
                    ),
                )
            })
        })
        .collect()
}
