pub mod cpp;
pub mod infra;
pub mod proto;
pub mod python;
pub mod rust;
pub mod typescript;

use crate::core::pack::Pack;

// The repository resolved whole: every pack composed into the one graph.
pub fn graphed(asked: &str) -> Option<crate::core::graph::Graph> {
    crate::core::graph::graph(asked, all())
}
use crate::packs::cpp::Cpp;
use crate::packs::infra::Infra;
use crate::packs::proto::Proto;
use crate::packs::python::Python;
use crate::packs::rust::Rust;
use crate::packs::typescript::TypeScript;

pub fn all() -> Vec<Box<dyn Pack>> {
    vec![
        Box::new(Cpp::new()),
        Box::new(Infra::new()),
        Box::new(Proto::new()),
        Box::new(Python::new()),
        Box::new(Rust::new()),
        Box::new(TypeScript::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::all;
    use crate::core::rule::PER_LANGUAGE;

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
