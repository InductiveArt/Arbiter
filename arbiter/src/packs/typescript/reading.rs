use std::collections::HashSet;
use std::path::PathBuf;

use crate::core::suppression::Suppression;

// What the TypeScript pack read from one file: what it imports and
// re-exports, what the laws read in it, and the shapes interfaces are
// satisfied through.
#[derive(Default)]
pub struct Reading {
    pub file: PathBuf,
    pub imports: Vec<Import>,
    // The lines holding a type-only import, which erases at build time.
    pub typed: HashSet<usize>,
    pub reexports: Vec<Reexport>,
    pub suppressions: Vec<Suppression>,
    pub aliases: Vec<Alias>,
    pub computed: Vec<Computed>,
    pub crowded: Vec<CrowdedImport>,
    pub overreaches: Vec<Overreach>,
    pub tautologies: Vec<Tautology>,
    pub doors: Doors,
}

// A name taken from a module specifier.
pub struct Import {
    pub from: String,
    pub name: String,
    pub line: usize,
}

// A re-export from a module specifier, a star forwarding every name.
pub struct Reexport {
    pub from: String,
    pub line: usize,
    pub star: bool,
}

// A name an import binds under another name.
pub struct Alias {
    pub alias: String,
    pub line: usize,
}

// A module imported through a value only run time resolves.
pub struct Computed {
    pub line: usize,
}

// An import statement binding more than one name.
pub struct CrowdedImport {
    pub names: usize,
    pub line: usize,
}

// A name a comment gives that the file neither defines, uses nor quotes.
pub struct Overreach {
    pub token: String,
    pub line: usize,
}

// The value an assertion compares with itself.
pub struct Tautology {
    pub text: String,
    pub line: usize,
}

// The shapes an interface is satisfied through: the interfaces, every class
// with its named heritage, each annotated parameter, and each construction
// standing at an annotated site.
#[derive(Default)]
pub struct Doors {
    pub interfaces: Vec<String>,
    pub classes: Vec<Class>,
    pub askings: Vec<Asking>,
    pub assigns: Vec<Assign>,
    pub callargs: Vec<CallArg>,
}

pub struct Class {
    pub name: String,
    pub bases: Vec<String>,
    pub line: usize,
}

// One annotated parameter: its function, its position, and the type names
// its annotation asks for.
pub struct Asking {
    pub function: String,
    pub place: usize,
    pub types: Vec<String>,
}

// A construction declared where an annotation names the types it must be.
pub struct Assign {
    pub types: Vec<String>,
    pub construction: String,
}

// A construction passed to a call at a position among its arguments.
pub struct CallArg {
    pub callee: String,
    pub place: usize,
    pub construction: String,
}
