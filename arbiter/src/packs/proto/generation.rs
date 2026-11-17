use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

use crate::core::drift::Drift;
use crate::core::language::marked_generated;
use crate::packs::proto::buf::run_buf;

// What protobuf code generators name their output, in every language a pack reads.
const GENERATED: [&str; 5] = ["_pb2.py", "_pb2.pyi", "_pb2_grpc.py", "_pb.ts", ".pb.ts"];

// The committed generated code of a unit with a generation config, held to
// what its contracts generate now. Regeneration reaches plugins over the
// network, so an unchanged cone of contracts skips it the way warm skips a
// green tool.
pub fn drift(root: &Path, folder: &Path, fixing: bool, cold: bool) -> Option<Drift> {
    if !folder.join("buf.gen.yaml").is_file() {
        return None;
    }
    let digest = generation_digest(folder);
    let kept = digest_path(root, folder);
    let warm = !cold
        && std::fs::read_to_string(&kept).is_ok_and(|stored| stored.trim() == digest.to_string());
    if warm {
        return Some(Drift {
            check: "stale-generated".to_string(),
            problems: Vec::new(),
            seconds: 0.0,
        });
    }
    let started = Instant::now();
    let problems = regenerated(folder, fixing);
    if problems.is_empty() {
        if let Some(parent) = kept.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&kept, format!("{digest}\n"));
    } else {
        let _ = std::fs::remove_file(&kept);
    }
    Some(Drift {
        check: "stale-generated".to_string(),
        problems,
        seconds: started.elapsed().as_secs_f64(),
    })
}

// One number over the generation's whole input cone: every contract, the
// generation config and the module config, in a stable order.
fn generation_digest(folder: &Path) -> u64 {
    let mut names: Vec<PathBuf> = Vec::new();
    for file in ["buf.yaml", "buf.gen.yaml"] {
        names.push(folder.join(file));
    }
    let mut contracts: Vec<PathBuf> = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(place) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&place) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if kind.is_dir() {
                if !name.starts_with('.') && name != "node_modules" {
                    stack.push(entry.path());
                }
            } else if name.ends_with(".proto") {
                contracts.push(entry.path());
            }
        }
    }
    contracts.sort();
    names.extend(contracts);
    let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
    for name in names {
        for byte in name.to_string_lossy().as_bytes() {
            digest ^= u64::from(*byte);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let Ok(bytes) = std::fs::read(&name) else {
            continue;
        };
        for byte in bytes {
            digest ^= u64::from(byte);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    digest
}

// Where the last clean generation's digest rests, beside the warm store.
fn digest_path(root: &Path, folder: &Path) -> PathBuf {
    let unit = folder.strip_prefix(root).unwrap_or(folder);
    let flat = if unit.as_os_str().is_empty() {
        "root".to_string()
    } else {
        unit.to_string_lossy().replace(['/', '\\'], "-")
    };
    root.join(".arbiter")
        .join("warm")
        .join(format!("generated-{flat}.fnv"))
}

// Regenerates to a staging folder and names every committed file that disagrees.
// A fixing run first regenerates in place and deletes the orphans.
fn regenerated(folder: &Path, fixing: bool) -> Vec<String> {
    if fixing {
        let _ = run_buf(folder, &["generate"]);
    }
    let staged = std::env::temp_dir()
        .join("arbiter-gen")
        .join(folder.to_string_lossy().replace(['/', '\\', ':'], "-"));
    let _ = std::fs::remove_dir_all(&staged);
    if std::fs::create_dir_all(&staged).is_err() {
        return vec!["the staging folder could not be created".to_string()];
    }
    if let Err(output) = run_buf(folder, &["generate", "--output", &staged.to_string_lossy()]) {
        return vec![format!("buf generate failed: {output}")];
    }
    if fixing {
        for orphan in orphans(&staged, folder) {
            let _ = std::fs::remove_file(folder.join(orphan));
        }
    }
    compared(&staged, folder)
}

fn compared(generated: &Path, committed: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    for file in files_under(generated, generated) {
        let theirs = std::fs::read(generated.join(&file)).unwrap_or_default();
        match std::fs::read(committed.join(&file)) {
            Err(_) => problems.push(format!(
                "{} is missing; the contracts generate it",
                file.to_string_lossy()
            )),
            Ok(ours) if ours != theirs => problems.push(format!(
                "{} drifted from what the contracts generate",
                file.to_string_lossy()
            )),
            Ok(_) => {}
        }
    }
    for orphan in orphans(generated, committed) {
        problems.push(format!(
            "{} is orphaned; no contract generates it",
            orphan.to_string_lossy()
        ));
    }
    problems.sort();
    problems
}

// Committed files named like generated code, or Rust marked as generated,
// that no contract generates any more.
fn orphans(generated: &Path, committed: &Path) -> Vec<PathBuf> {
    let fresh: std::collections::HashSet<PathBuf> =
        files_under(generated, generated).into_iter().collect();
    files_under(committed, committed)
        .into_iter()
        .filter(|file| {
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            let rust = name.ends_with(".rs")
                && std::fs::read_to_string(committed.join(file))
                    .is_ok_and(|source| marked_generated(&source));
            (rust || GENERATED.iter().any(|suffix| name.ends_with(suffix))) && !fresh.contains(file)
        })
        .collect()
}

fn files_under(base: &Path, folder: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return found;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        if kind.is_dir() {
            if name.starts_with('.')
                || ["node_modules", "dist", "target", "__pycache__"].contains(&name.as_str())
            {
                continue;
            }
            found.extend(files_under(base, &path));
        } else if kind.is_file() {
            found.push(path.strip_prefix(base).unwrap_or(&path).to_path_buf());
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::compared;

    fn planted(base: &Path, file: &str, content: &str) {
        let path = base.join(file);
        fs::create_dir_all(path.parent().expect("a planted file has a folder"))
            .expect("the folder is writable");
        fs::write(path, content).expect("the file is writable");
    }

    #[test]
    fn a_missing_a_drifted_and_an_orphaned_file_are_each_named() {
        let base = std::env::temp_dir().join("arbiter-test-compared");
        let _ = fs::remove_dir_all(&base);
        let generated = base.join("generated");
        let committed = base.join("committed");
        planted(&generated, "src/gen/same_pb.ts", "same");
        planted(&generated, "src/gen/stale_pb.ts", "new");
        planted(&generated, "src/gen/fresh_pb.ts", "fresh");
        planted(&committed, "src/gen/same_pb.ts", "same");
        planted(&committed, "src/gen/stale_pb.ts", "old");
        planted(&committed, "src/gen/ghost_pb.ts", "ghost");
        planted(&committed, "src/own.ts", "handwritten");
        let problems = compared(&generated, &committed);
        assert_eq!(
            problems,
            [
                "src/gen/fresh_pb.ts is missing; the contracts generate it",
                "src/gen/ghost_pb.ts is orphaned; no contract generates it",
                "src/gen/stale_pb.ts drifted from what the contracts generate",
            ]
        );
    }

    #[test]
    fn a_rust_file_marked_generated_that_no_contract_generates_is_orphaned() {
        let base = std::env::temp_dir().join("arbiter-test-orphaned-rust");
        let _ = fs::remove_dir_all(&base);
        let generated = base.join("generated");
        let committed = base.join("committed");
        planted(&generated, "src/contract/app.v1.rs", "// @generated\nkept");
        planted(&committed, "src/contract/app.v1.rs", "// @generated\nkept");
        planted(&committed, "src/contract/old.v1.rs", "// @generated\nold");
        planted(
            &committed,
            "src/contract/mod.rs",
            "include!(\"app.v1.rs\");\n",
        );
        assert_eq!(
            compared(&generated, &committed),
            ["src/contract/old.v1.rs is orphaned; no contract generates it"]
        );
    }
}
