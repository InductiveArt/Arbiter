use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::core::suppression::Suppression;

// What the Python pack read from one file: what it imports and how, what the
// laws read in it, and the shapes protocols are satisfied through.
#[derive(Default, Serialize, Deserialize)]
pub struct Reading {
    pub file: PathBuf,
    pub imports: Vec<Import>,
    // The names a package's init file imports while being a surface, which
    // it then offers as its own.
    pub init_imports: Vec<InitImport>,
    pub plains: Vec<Plain>,
    pub chains: Vec<Chain>,
    pub computed: Vec<Computed>,
    pub suppressions: Vec<Suppression>,
    pub aliases: Vec<Alias>,
    pub overreaches: Vec<Overreach>,
    pub tautologies: Vec<Tautology>,
    pub doors: Doors,
}

// A name taken from a module, as a from-import or a star import takes it.
#[derive(Serialize, Deserialize)]
pub struct Import {
    pub module: String,
    pub name: String,
    pub line: usize,
}

#[derive(Serialize, Deserialize)]
pub struct InitImport {
    pub name: String,
    pub line: usize,
}

// A module imported whole, by a plain import or a loader called with a literal.
#[derive(Serialize, Deserialize)]
pub struct Plain {
    pub module: String,
    pub line: usize,
}

// A dotted chain of names read off a module imported whole.
#[derive(Serialize, Deserialize)]
pub struct Chain {
    pub chain: String,
    pub line: usize,
}

// A module imported through a value only run time resolves.
#[derive(Serialize, Deserialize)]
pub struct Computed {
    pub line: usize,
}

// A name an import binds under another name.
#[derive(Serialize, Deserialize)]
pub struct Alias {
    pub alias: String,
    pub line: usize,
}

// A name a comment gives that the file neither defines, uses nor quotes.
#[derive(Serialize, Deserialize)]
pub struct Overreach {
    pub token: String,
    pub line: usize,
}

// An assertion that holds whatever the code does.
#[derive(Serialize, Deserialize)]
pub struct Tautology {
    pub text: String,
    pub line: usize,
}

// The shapes a protocol is satisfied through: the protocol classes, every
// class with its bases, each annotated parameter, and each constructor
// standing at an annotated site.
#[derive(Default, Serialize, Deserialize)]
pub struct Doors {
    pub protocols: Vec<String>,
    pub classes: Vec<Class>,
    pub askings: Vec<Asking>,
    pub assigns: Vec<Assign>,
    pub callargs: Vec<CallArg>,
}

#[derive(Serialize, Deserialize)]
pub struct Class {
    pub name: String,
    pub bases: Vec<String>,
    pub line: usize,
}

// One annotated parameter: its function, its position, its keyword, and the
// type names its annotation asks for.
#[derive(Serialize, Deserialize)]
pub struct Asking {
    pub function: String,
    pub place: usize,
    pub keyword: Option<String>,
    pub types: Vec<String>,
}

// A constructor assigned where an annotation names the types it must be.
#[derive(Serialize, Deserialize)]
pub struct Assign {
    pub types: Vec<String>,
    pub constructor: String,
}

// A constructor passed to a call, by its position among the arguments or by
// its keyword.
#[derive(Serialize, Deserialize)]
pub struct CallArg {
    pub callee: String,
    pub place: usize,
    pub keyword: Option<String>,
    pub constructor: String,
}
