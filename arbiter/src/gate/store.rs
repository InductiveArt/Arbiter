use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

// Where the gate keeps what its runs computed: each run's record, the outputs
// and the cache naming them. Git never carries it, and deleting it is the
// full chill.
pub fn opened(root: &Path) -> io::Result<PathBuf> {
    let store = root.join(".arbiter").join("warm");
    fs::create_dir_all(&store)?;
    fs::write(store.join(".gitignore"), "*\n")?;
    Ok(store)
}
