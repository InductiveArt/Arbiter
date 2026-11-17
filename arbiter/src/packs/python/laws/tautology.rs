use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::domain::finding::Finding;
use crate::packs::python::reading::Reading;

// A fixture's tautologies are some test's data, never findings.
pub fn instances(readings: &[Reading], repo: &Repo) -> Vec<Finding> {
    readings
        .iter()
        .filter(|reading| {
            let unit = repo.unit_of(&reading.file);
            !is_test(&repo.units[unit].path)
        })
        .flat_map(|reading| {
            reading.tautologies.iter().map(|tautology| {
                Finding::integrity(
                    &reading.file,
                    tautology.line,
                    "tautology",
                    format!(
                        "{}; an assertion compares the system to an expectation",
                        tautology.text
                    ),
                )
            })
        })
        .collect()
}
