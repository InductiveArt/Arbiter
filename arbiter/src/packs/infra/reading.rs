use std::path::PathBuf;

// What the infra pack read from one manifest: each image it pulls by tag,
// and each name a comment gives that nothing in it carries.
#[derive(Default)]
pub struct Reading {
    pub file: PathBuf,
    pub floating: Vec<FloatingImage>,
    pub overreaches: Vec<Overreach>,
}

// An image a manifest pulls by tag alone.
pub struct FloatingImage {
    pub image: String,
    pub line: usize,
}

// A name a comment gives that the manifest neither defines, uses nor quotes.
pub struct Overreach {
    pub token: String,
    pub line: usize,
}
