use crate::domain::finding::Finding;
use crate::packs::python::reading::Reading;

pub fn instances(readings: &[Reading]) -> Vec<Finding> {
    readings
        .iter()
        .flat_map(|reading| {
            reading.overreaches.iter().map(|overreach| {
                Finding::integrity(
                    &reading.file,
                    overreach.line,
                    "comment-overreach",
                    format!(
                        "a comment names {}, which nothing in the file defines, uses or quotes",
                        overreach.token
                    ),
                )
            })
        })
        .collect()
}
