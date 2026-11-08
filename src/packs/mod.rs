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
