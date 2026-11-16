use crate::domain::finding::Finding;
use crate::packs::infra::reading::Reading;

pub fn instances(readings: &[Reading]) -> Vec<Finding> {
    readings
        .iter()
        .flat_map(|reading| {
            reading.floating.iter().map(|floating| {
                Finding::integrity(
                    &reading.file,
                    floating.line,
                    "unpinned-image",
                    format!(
                        "{} is pulled by tag; an image is named by digest, never by a tag alone",
                        floating.image
                    ),
                )
            })
        })
        .collect()
}
