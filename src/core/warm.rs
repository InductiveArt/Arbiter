use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::pack::{Edge, Pack};
use crate::core::units::Repo;

// 128-bit FNV-1a: stable across runs and platforms, fast, and wide enough for
// a local cache whose adversary is chance, never an attacker.
pub struct Digest(u128);

const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
const PRIME: u128 = 0x0000000001000000000000000000013b;

impl Digest {
    pub fn new() -> Self {
        Digest(OFFSET)
    }

    pub fn eat(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u128::from(*byte);
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }

    pub fn hex(&self) -> String {
        format!("{:032x}", self.0)
    }
}

impl Default for Digest {
    fn default() -> Self {
        Self::new()
    }
}

// What a warm skip stands on: the unit's whole cone of inputs, keyed by
// content. The store holds only green runs, and it never commits, so a fresh
// clone is cold by construction.
pub struct Warmth {
    folder: PathBuf,
    pub keys: Vec<Option<String>>,
    reading: bool,
}

const STORE: &str = ".arbiter/warm";

pub fn warmth(
    root: &Path,
    repo: &Repo,
    packs: &[Box<dyn Pack>],
    edges: &[Edge],
    defs_units: &[usize],
) -> Warmth {
    // The packs' own identity: a changed adapter, config or binary colds everything.
    let mut packs_digest = Digest::new();
    packs_digest.eat(env!("CARGO_PKG_VERSION").as_bytes());
    for pack in packs {
        packs_digest.eat(pack.label().as_bytes());
        if let Some(adapter) = pack.adapter() {
            packs_digest.eat(adapter.as_bytes());
        }
        for (name, content) in pack.configs() {
            packs_digest.eat(name.as_bytes());
            packs_digest.eat(content.as_bytes());
        }
    }
    let mut locks_digest = Digest::new();
    for lock in &repo.lockfiles {
        locks_digest.eat(lock.to_string_lossy().as_bytes());
        if let Ok(text) = fs::read(root.join(lock)) {
            locks_digest.eat(&text);
        }
    }
    let shared = format!("{}{}", packs_digest.hex(), locks_digest.hex());

    // Each unit's own folder, hashed once; the key then folds in its closure.
    let paths: Vec<&PathBuf> = repo.units.iter().map(|unit| &unit.path).collect();
    let own: Vec<Option<String>> = repo
        .units
        .iter()
        .enumerate()
        .map(|(index, unit)| {
            if repo.is_test(index, &unit.path) {
                return None;
            }
            let mut digest = Digest::new();
            eaten(
                root,
                &root.join(&unit.path),
                &unit.path,
                &paths,
                &mut digest,
            );
            Some(digest.hex())
        })
        .collect();

    let reach = closure(repo.units.len(), edges, defs_units);
    let keys = own
        .iter()
        .enumerate()
        .map(|(index, mine)| {
            let mine = mine.as_ref()?;
            let mut digest = Digest::new();
            digest.eat(shared.as_bytes());
            digest.eat(mine.as_bytes());
            for &dep in &reach[index] {
                if let Some(theirs) = &own[dep] {
                    digest.eat(theirs.as_bytes());
                }
            }
            Some(digest.hex())
        })
        .collect();
    Warmth {
        folder: root.join(STORE),
        keys,
        reading: true,
    }
}

// The units a unit stands on, transitively, read from the resolved edges.
fn closure(count: usize, edges: &[Edge], defs_units: &[usize]) -> Vec<BTreeSet<usize>> {
    let mut direct = vec![BTreeSet::new(); count];
    for edge in edges {
        let owner = defs_units[edge.def];
        if owner != edge.unit {
            direct[edge.unit].insert(owner);
        }
    }
    (0..count)
        .map(|start| {
            let mut seen = BTreeSet::new();
            let mut queue: Vec<usize> = direct[start].iter().copied().collect();
            while let Some(at) = queue.pop() {
                if seen.insert(at) {
                    queue.extend(direct[at].iter().copied());
                }
            }
            seen
        })
        .collect()
}

// Every file in the folder git would carry, except another unit's and the
// ecosystems' outputs; a config beside the manifest counts as much as source.
fn eaten(root: &Path, folder: &Path, unit: &Path, units: &[&PathBuf], digest: &mut Digest) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    let mut sorted: Vec<_> = entries.flatten().collect();
    sorted.sort_by_key(std::fs::DirEntry::file_name);
    for entry in sorted {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        if kind.is_dir() {
            if name.starts_with('.')
                || name == "node_modules"
                || name == "dist"
                || name == "target"
                || name == "third_party"
                || units
                    .iter()
                    .any(|other| **other == relative && **other != *unit)
            {
                continue;
            }
            eaten(root, &path, unit, units, digest);
        } else if kind.is_file() {
            digest.eat(relative.to_string_lossy().as_bytes());
            if let Ok(bytes) = fs::read(&path) {
                digest.eat(&bytes);
            }
        }
    }
}

fn slug(unit: &Path) -> String {
    unit.to_string_lossy().replace(['/', '\\'], "__")
}

pub struct StoredCheck {
    pub check: String,
    pub pack: String,
    pub seconds: f64,
}

impl Warmth {
    // A chilled store answers no reads; records still land.
    pub fn chill(&mut self) {
        self.reading = false;
    }

    // The green run stored under today's key, or nothing to skip with.
    pub fn stored(&self, index: usize, unit: &Path) -> Option<Vec<StoredCheck>> {
        if !self.reading {
            return None;
        }
        let key = self.keys.get(index)?.as_ref()?;
        let text = fs::read_to_string(self.folder.join(format!("{}.json", slug(unit)))).ok()?;
        let parsed: serde_json::Value = serde_json::from_str(&text).ok()?;
        if parsed.get("key")?.as_str()? != key {
            return None;
        }
        let checks = parsed.get("checks")?.as_array()?;
        let mut found = Vec::new();
        for check in checks {
            found.push(StoredCheck {
                check: check.get("check")?.as_str()?.to_string(),
                pack: check.get("pack")?.as_str()?.to_string(),
                seconds: check.get("seconds")?.as_f64()?,
            });
        }
        Some(found)
    }

    pub fn record(&self, index: usize, unit: &Path, checks: &[StoredCheck]) {
        let Some(Some(key)) = self.keys.get(index) else {
            return;
        };
        if fs::create_dir_all(&self.folder).is_err() {
            return;
        }
        let _ = fs::write(self.folder.join(".gitignore"), "*\n");
        let listed: Vec<serde_json::Value> = checks
            .iter()
            .map(|check| {
                serde_json::json!({
                    "check": check.check,
                    "pack": check.pack,
                    "seconds": check.seconds,
                })
            })
            .collect();
        let entry = serde_json::json!({ "key": key, "checks": listed });
        let _ = fs::write(
            self.folder.join(format!("{}.json", slug(unit))),
            entry.to_string(),
        );
    }

    // A red unit's record goes: a failure is re-proven every run until green.
    pub fn invalidate(&self, unit: &Path) {
        let _ = fs::remove_file(self.folder.join(format!("{}.json", slug(unit))));
    }
}
