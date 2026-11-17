use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

// The files git carries under the folder, by path from it. Git is asked once
// and only this: with no git, or no repository around the folder, nothing is
// known to be carried, and the folder's own .gitignore files alone say what
// is out.
pub fn committed(root: &Path) -> BTreeSet<PathBuf> {
    let Ok(listed) = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z", "--cached"])
        .output()
    else {
        return BTreeSet::new();
    };
    if !listed.status.success() {
        return BTreeSet::new();
    }
    listed
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(OsStr::from_bytes(path)))
        .collect()
}
