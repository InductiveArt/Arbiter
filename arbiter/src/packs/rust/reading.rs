use std::path::PathBuf;

use crate::core::suppression::Suppression;

// What the Rust pack read from one file: the names its paths reach, what it
// declares and takes in, and what the laws read in it.
#[derive(Default)]
pub struct Reading {
    pub file: PathBuf,
    // Whether a generator wrote the file: read for what it uses, nothing more.
    pub generated: bool,
    pub uses: Vec<Use>,
    pub entries: Vec<Entry>,
    pub imports: Vec<Import>,
    // The modules the file declares, with a body or in a file of their own.
    pub modules: Vec<String>,
    // The files it takes in whole, by path from the repository.
    pub includes: Vec<PathBuf>,
    pub suppressions: Vec<Suppression>,
    pub aliases: Vec<Alias>,
    pub reexports: Vec<Reexport>,
    pub overreaches: Vec<Overreach>,
    pub tautologies: Vec<Tautology>,
    pub crowded: Vec<CrowdedUse>,
}

// A name reached through the module path written before it.
pub struct Use {
    pub module: String,
    pub name: String,
    pub line: usize,
}

// The first segment of a use declaration: where an external crate enters.
pub struct Entry {
    pub first: String,
    pub line: usize,
}

// A name a use declaration binds, with the module it binds it from.
pub struct Import {
    pub name: String,
    pub from: String,
}

// A name a use declaration binds under another name.
pub struct Alias {
    pub alias: String,
    pub line: usize,
}

// A use declaration offering another module's name as the file's own.
pub struct Reexport {
    pub argument: String,
    pub line: usize,
}

// A name a comment gives that the file neither defines, uses nor quotes.
pub struct Overreach {
    pub token: String,
    pub line: usize,
}

// An assertion that holds whatever the code does.
pub struct Tautology {
    pub text: String,
    pub line: usize,
}

// A use declaration binding more than one name on its line.
pub struct CrowdedUse {
    pub names: usize,
    pub line: usize,
}
