use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::value::encoded_value::EncodedValue;
use crate::value::task_output_hash::TaskOutputHash;

// The outputs tasks left, each kept once in one folder under the hash of its
// bytes, whatever run or attempt left it.
pub struct TaskOutputStore {
    dir: PathBuf,
}

impl TaskOutputStore {
    pub fn at(dir: PathBuf) -> TaskOutputStore {
        TaskOutputStore { dir }
    }

    // Moves a file an attempt left into the store under its hash. The same
    // bytes left twice are kept once.
    pub fn keep(&self, left: &Path, hash: &TaskOutputHash) -> io::Result<PathBuf> {
        fs::create_dir_all(&self.dir)?;
        let kept = self.dir.join(hash.as_str());
        fs::rename(left, &kept)?;
        Ok(kept)
    }

    // The output kept under the hash, given back only while its bytes still
    // hash to it.
    pub fn read(&self, hash: &TaskOutputHash) -> io::Result<EncodedValue> {
        let value = EncodedValue::from_file(self.dir.join(hash.as_str()))?;
        if TaskOutputHash::of(&value.bytes) == *hash {
            Ok(value)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "the output kept as {} no longer hashes to it",
                    hash.as_str()
                ),
            ))
        }
    }

    // Removes every output the set does not name. Only files named as a hash
    // are touched.
    pub fn prune(&self, named: &HashSet<TaskOutputHash>) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(hash) = file_name.to_str().and_then(TaskOutputHash::parse) else {
                continue;
            };
            if !named.contains(&hash) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::env;
    use std::fs;
    use std::process;

    use super::TaskOutputStore;
    use crate::value::task_output_hash::TaskOutputHash;

    #[test]
    fn an_output_is_kept_under_its_hash_read_back_and_refused_once_its_bytes_change() {
        let dir = env::temp_dir().join(format!("task-runner-outputs-{}", process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a folder is made");
        let left = dir.join("output.json");
        fs::write(&left, b"[1,2]").expect("an output is left");
        let hash = TaskOutputHash::of(b"[1,2]");
        let store = TaskOutputStore::at(dir.join("outputs"));
        let kept = store.keep(&left, &hash).expect("kept");
        assert!(!left.exists(), "the output moved into the store");
        assert_eq!(kept, dir.join("outputs").join(hash.as_str()));
        assert_eq!(&*store.read(&hash).expect("read back").bytes, b"[1,2]");
        fs::write(&kept, b"[1,3]").expect("the file is changed");
        assert!(
            store.read(&hash).is_err(),
            "bytes that no longer hash to the name are refused"
        );
        let other = TaskOutputHash::of(b"other");
        fs::write(dir.join("outputs").join(other.as_str()), b"other").expect("written");
        fs::write(dir.join("outputs").join("notes"), b"kept").expect("written");
        store.prune(&HashSet::from([hash.clone()]));
        assert!(kept.exists());
        assert!(!dir.join("outputs").join(other.as_str()).exists());
        assert!(
            dir.join("outputs").join("notes").exists(),
            "only hashes are touched"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
