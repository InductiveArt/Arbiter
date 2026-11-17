use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use crate::core::definition::Def;
use crate::core::digest::Digest;
use crate::core::edge::Edge;
use crate::core::repository::Repo;
use crate::domain::unit::Unit;
use crate::domain::unit_cone::UnitCone;

// Each unit's input cone, in the order the units stand: its own files, the
// files of every unit it stands on through the graph, however far, and every
// lockfile, each file by its path and the digest of its content. Nothing is
// read from disk: the listing already holds each file's digest.
pub fn cones(repo: &Repo, defs: &[Def], edges: &[Edge]) -> Vec<UnitCone> {
    let listed: BTreeMap<&Path, &str> = repo
        .units
        .iter()
        .flat_map(|unit| unit.files.iter())
        .map(|file| (file.path.as_path(), file.digest.as_str()))
        .collect();
    let mut locks = Digest::new();
    for lock in &repo.lockfiles {
        locks.eat(lock.to_string_lossy().as_bytes());
        locks.eat(
            listed
                .get(lock.as_path())
                .copied()
                .unwrap_or_default()
                .as_bytes(),
        );
    }
    let locks = locks.hex();
    let own: Vec<String> = repo.units.iter().map(own).collect();
    let reach = stands_on(repo.units.len(), defs, edges);
    repo.units
        .iter()
        .enumerate()
        .map(|(at, unit)| {
            let mut digest = Digest::new();
            digest.eat(locks.as_bytes());
            digest.eat(own[at].as_bytes());
            for &other in &reach[at] {
                digest.eat(own[other].as_bytes());
            }
            UnitCone {
                unit: unit.path.clone(),
                digest: digest.hex(),
            }
        })
        .collect()
}

// The unit's own files, in the order of their paths.
fn own(unit: &Unit) -> String {
    let mut files: Vec<_> = unit.files.iter().collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut digest = Digest::new();
    for file in files {
        digest.eat(file.path.to_string_lossy().as_bytes());
        digest.eat(file.digest.as_bytes());
    }
    digest.hex()
}

// The units each unit stands on, however far, read from the edges: a use in
// one unit of a definition in another.
fn stands_on(count: usize, defs: &[Def], edges: &[Edge]) -> Vec<BTreeSet<usize>> {
    let mut direct = vec![BTreeSet::new(); count];
    for edge in edges {
        let owner = defs[edge.def].unit;
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::cones;
    use crate::core::definition::Def;
    use crate::core::edge::Edge;
    use crate::core::repository::Repo;
    use crate::domain::file::File;
    use crate::domain::unit::Unit;

    // Three units: the top one uses a definition of `a`, which uses one of
    // `a/deep`; `b` stands alone. Each file is written as its path and digest.
    fn repo(files: &[(&str, &str)]) -> Repo {
        let units = ["", "a", "a/deep", "b"]
            .iter()
            .map(|path| Unit {
                path: PathBuf::from(path),
                markers: Vec::new(),
                files: files
                    .iter()
                    .filter(|(file, _)| {
                        PathBuf::from(file).parent().map(PathBuf::from) == Some(PathBuf::from(path))
                    })
                    .map(|(file, digest)| File {
                        path: PathBuf::from(file),
                        bytes: 1,
                        digest: digest.to_string(),
                    })
                    .collect(),
            })
            .collect();
        Repo {
            units,
            roots: BTreeSet::new(),
            files: files.iter().map(|(file, _)| PathBuf::from(file)).collect(),
            manifests: Vec::new(),
            lockfiles: vec![PathBuf::from("Cargo.lock")],
            published: BTreeSet::new(),
            foreign: Vec::new(),
        }
    }

    fn def(unit: usize) -> Def {
        Def {
            name: "x".to_string(),
            file: PathBuf::new(),
            line: 1,
            unit,
            pack: 0,
            self_used: false,
        }
    }

    fn edge(def: usize, unit: usize) -> Edge {
        Edge {
            def,
            file: PathBuf::new(),
            line: 1,
            unit,
        }
    }

    const FILES: [(&str, &str); 5] = [
        ("Cargo.lock", "l1"),
        ("top.rs", "t1"),
        ("a/a.rs", "a1"),
        ("a/deep/d.rs", "d1"),
        ("b/b.rs", "b1"),
    ];

    // Each unit's digest, the top unit standing on `a`, and `a` on `a/deep`.
    fn digests(files: &[(&str, &str)]) -> Vec<String> {
        cones(&repo(files), &[def(1), def(2)], &[edge(0, 0), edge(1, 1)])
            .into_iter()
            .map(|cone| cone.digest)
            .collect()
    }

    fn changed(file: &str) -> Vec<bool> {
        let before = digests(&FILES);
        let mut after = FILES;
        for entry in &mut after {
            if entry.0 == file {
                entry.1 = "changed";
            }
        }
        before
            .iter()
            .zip(digests(&after))
            .map(|(was, now)| *was != now)
            .collect()
    }

    #[test]
    fn a_cone_follows_the_graph_however_far_and_names_each_unit() {
        let found = cones(&repo(&FILES), &[def(1), def(2)], &[edge(0, 0), edge(1, 1)]);
        let units: Vec<&str> = found
            .iter()
            .map(|cone| cone.unit.to_str().expect("a path in UTF-8"))
            .collect();
        assert_eq!(units, ["", "a", "a/deep", "b"]);
        assert_eq!(
            digests(&FILES),
            digests(&FILES),
            "the same files, the same cones"
        );
        assert_eq!(
            changed("a/deep/d.rs"),
            [true, true, true, false],
            "a file reaches every unit standing on its own, however far"
        );
        assert_eq!(
            changed("top.rs"),
            [true, false, false, false],
            "a file reaches no unit its own unit stands on"
        );
        assert_eq!(changed("b/b.rs"), [false, false, false, true]);
    }

    #[test]
    fn a_lockfile_is_in_every_cone() {
        assert_eq!(changed("Cargo.lock"), [true, true, true, true]);
    }

    #[test]
    fn a_use_inside_its_own_unit_widens_no_cone() {
        let alone = cones(&repo(&FILES), &[def(3)], &[edge(0, 3)]);
        let none = cones(&repo(&FILES), &[], &[]);
        assert_eq!(alone, none);
    }
}
