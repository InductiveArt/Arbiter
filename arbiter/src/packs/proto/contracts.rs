use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

use crate::core::drift::Drift;
use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::packs::proto::buf::run_buf;
use crate::packs::proto::language::MANIFEST;

// A unit's contracts held to the image the user signed: a break between
// languages ships only with a new signature. A unit with no image yet is
// told how to record one.
pub fn drift(root: &Path, folder: &Path, notes: &mut Vec<String>) -> Option<Drift> {
    if !folder.join(MANIFEST).is_file() {
        return None;
    }
    let image = image_path(root, folder);
    if !image.is_file() {
        let shown = folder.strip_prefix(root).unwrap_or(folder);
        notes.push(format!(
            "{} has no contracts reference; arbiter ledger sign records today's",
            shown.to_string_lossy()
        ));
        return None;
    }
    let started = Instant::now();
    let problems = match run_buf(folder, &["breaking", "--against", &image.to_string_lossy()]) {
        Ok(_) => Vec::new(),
        Err(output) => output.lines().map(str::to_string).collect(),
    };
    Some(Drift {
        check: "inter-language-break".to_string(),
        problems,
        seconds: started.elapsed().as_secs_f64(),
    })
}

// Records each unit's contracts as the image later runs are held to: the
// user's signature.
pub fn record(root: &Path, repo: &Repo) -> Vec<String> {
    let mut lines = Vec::new();
    for manifest in &repo.manifests {
        if manifest.file_name().is_none_or(|name| name != MANIFEST) {
            continue;
        }
        let unit = manifest.parent().unwrap_or(Path::new(""));
        // A unit on a test path is a fixture: data for some test, not contracts to record.
        if is_test(unit) {
            continue;
        }
        let folder = root.join(unit);
        let image = image_path(root, &folder);
        if let Some(parent) = image.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let kept = image.strip_prefix(root).unwrap_or(&image).to_path_buf();
        match run_buf(&folder, &["build", "-o", &image.to_string_lossy()]) {
            Ok(_) => lines.push(format!(
                "contracts of {} recorded in {}",
                unit.to_string_lossy(),
                kept.to_string_lossy()
            )),
            Err(output) => lines.push(format!(
                "contracts of {} could not be recorded: {output}",
                unit.to_string_lossy()
            )),
        }
    }
    lines
}

// Where the signed contracts of the unit live, named by the unit's own path.
fn image_path(root: &Path, folder: &Path) -> PathBuf {
    let unit = folder.strip_prefix(root).unwrap_or(folder);
    let flat = if unit.as_os_str().is_empty() {
        "root".to_string()
    } else {
        unit.to_string_lossy().replace(['/', '\\'], "-")
    };
    root.join(".arbiter")
        .join("contracts")
        .join(format!("{flat}.binpb"))
}
