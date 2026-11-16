pub mod cpp;
pub mod infra;
pub mod proto;
pub mod python;
pub mod rust;
pub mod typescript;

use crate::core::pack::Pack;
use crate::core::pack::Packed;

// The repository resolved whole: every pack composed into the one graph.
pub fn graphed(asked: &str) -> Option<crate::core::graph::Graph> {
    crate::core::graph::graph(asked, all()).ok()
}
use crate::packs::cpp::Cpp;
use crate::packs::infra::Infra;
use crate::packs::proto::Proto;
use crate::packs::python::Python;
use crate::packs::rust::Rust;
use crate::packs::typescript::TypeScript;

pub fn all() -> Vec<Box<dyn Pack>> {
    vec![
        Box::new(Packed::new(Cpp)),
        Box::new(Packed::new(Infra)),
        Box::new(Packed::new(Proto)),
        Box::new(Packed::new(Python)),
        Box::new(Packed::new(Rust)),
        Box::new(Packed::new(TypeScript)),
    ]
}

#[cfg(test)]
mod tests {
    use super::all;
    use crate::core::ecosystems::ecosystems;
    use crate::core::rule::PER_LANGUAGE;

    #[test]
    fn every_language_marks_its_units_with_the_manifest_its_record_names() {
        let languages = all();
        let records = ecosystems(&languages).expect("the records read");
        for language in &languages {
            let record = records
                .iter()
                .find(|record| record.name == language.label());
            assert_eq!(
                language.manifest(),
                record.map(|record| record.marker.as_str()),
                "{}'s manifest and its record's marker differ",
                language.label()
            );
        }
    }

    #[test]
    fn every_pack_states_its_stance_on_every_rule_a_pack_answers() {
        for pack in all() {
            for rule in PER_LANGUAGE {
                assert!(
                    pack.stances().of(rule.name).is_some(),
                    "{} gives no stance on {}",
                    pack.label(),
                    rule.name
                );
            }
        }
    }
}
