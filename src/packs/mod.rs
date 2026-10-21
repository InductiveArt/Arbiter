pub mod cpp;
pub mod proto;
pub mod python;
pub mod rust;
pub mod typescript;

use crate::core::pack::Pack;
use crate::packs::cpp::Cpp;
use crate::packs::proto::Proto;
use crate::packs::python::Python;
use crate::packs::rust::Rust;
use crate::packs::typescript::TypeScript;

pub fn all() -> Vec<Box<dyn Pack>> {
    vec![
        Box::new(Cpp::new()),
        Box::new(Proto::new()),
        Box::new(Python::new()),
        Box::new(Rust::new()),
        Box::new(TypeScript::new()),
    ]
}
