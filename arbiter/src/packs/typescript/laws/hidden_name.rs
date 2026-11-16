use crate::domain::finding::Finding;
use crate::packs::typescript::reading::Reading;

// A name imported under another name, then a module imported through a
// computed value, each hiding what the import names.
pub fn instances(readings: &[Reading]) -> Vec<Finding> {
    let aliased = readings.iter().flat_map(|reading| {
        reading.aliases.iter().map(|alias| {
            Finding::integrity(
                &reading.file,
                alias.line,
                "hidden-name",
                format!("imported as {}; a symbol keeps its own name", alias.alias),
            )
        })
    });
    let computed = readings.iter().flat_map(|reading| {
        reading.computed.iter().map(|computed| {
            Finding::integrity(
                &reading.file,
                computed.line,
                "hidden-name",
                "imports a module through a computed value; a module is named by a literal"
                    .to_string(),
            )
        })
    });
    aliased.chain(computed).collect()
}
