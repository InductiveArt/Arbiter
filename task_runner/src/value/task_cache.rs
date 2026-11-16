use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::value::task_cache_key::TaskCacheKey;
use crate::value::task_output_hash::TaskOutputHash;

// What output a task left on given inputs under given code: one file per
// key, holding the hash of that output. It holds no output itself.
pub struct TaskCache {
    dir: PathBuf,
}

impl TaskCache {
    pub fn at(dir: PathBuf) -> TaskCache {
        TaskCache { dir }
    }

    // The output's hash the key names; nothing when no entry, or no whole
    // one, is there.
    pub fn get(&self, key: &TaskCacheKey) -> Option<TaskOutputHash> {
        let text = fs::read_to_string(self.dir.join(key.as_str())).ok()?;
        TaskOutputHash::parse(text.trim())
    }

    pub fn put(&self, key: &TaskCacheKey, hash: &TaskOutputHash) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        fs::write(self.dir.join(key.as_str()), hash.as_str())
    }

    // Removes every entry the set does not name. Only files named as a key
    // are touched.
    pub fn prune(&self, named: &HashSet<TaskCacheKey>) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(key) = file_name.to_str().and_then(TaskCacheKey::parse) else {
                continue;
            };
            if !named.contains(&key) {
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

    use super::TaskCache;
    use crate::value::task_cache_key::TaskCacheKey;
    use crate::value::task_output_hash::TaskOutputHash;

    #[test]
    fn a_cache_entry_names_an_output_by_hash_and_only_a_whole_one_is_read() {
        let dir = env::temp_dir().join(format!("task-runner-cache-{}", process::id()));
        let _ = fs::remove_dir_all(&dir);
        let cache = TaskCache::at(dir.clone());
        let key = TaskCacheKey::of(&[Some(b"scan")]);
        let hash = TaskOutputHash::of(b"[1,2]");
        assert_eq!(cache.get(&key), None);
        cache.put(&key, &hash).expect("written");
        assert_eq!(cache.get(&key), Some(hash.clone()));
        fs::write(dir.join(key.as_str()), &hash.as_str()[..10]).expect("cut short");
        assert_eq!(cache.get(&key), None, "half an entry reads as none");
        let other = TaskCacheKey::of(&[Some(b"lint")]);
        cache.put(&other, &hash).expect("written");
        cache.prune(&HashSet::from([key.clone()]));
        assert!(dir.join(key.as_str()).exists());
        assert!(!dir.join(other.as_str()).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
