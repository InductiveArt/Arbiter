use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::core::suppression::Suppression;

// What the proto pack read from one contract: the types it refers to, the
// contracts it imports, and the lint rules it quiets.
#[derive(Default, Serialize, Deserialize)]
pub struct Reading {
    pub file: PathBuf,
    pub typerefs: Vec<TypeRef>,
    // The contracts it imports, by the path written in the import.
    pub imports: Vec<String>,
    pub suppressions: Vec<Suppression>,
}

// A type named where a field or a method takes it, possibly qualified.
#[derive(Serialize, Deserialize)]
pub struct TypeRef {
    pub name: String,
    pub line: usize,
}
