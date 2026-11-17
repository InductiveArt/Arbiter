use crate::domain::finding::Finding;
use crate::packs::python::reading::Reading;

pub fn instances(readings: &[Reading]) -> Vec<Finding> {
    readings
        .iter()
        .flat_map(|reading| {
            reading.init_imports.iter().map(|import| {
                Finding::integrity(
                    &reading.file,
                    import.line,
                    "reexport",
                    format!(
                        "__init__.py imports {}; a symbol is imported from the module defining it",
                        import.name
                    ),
                )
            })
        })
        .collect()
}
