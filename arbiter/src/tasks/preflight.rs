use std::path::Path;
use std::process::Command;

// The machine's own gaps: every axiom a present pack stands on that does not
// answer. A gap is a broken machine, so the caller stops before judging anything.
pub fn preflight(root: &Path) -> Vec<String> {
    let packs = crate::packs::all();
    let repo = crate::core::units::scan(root, &packs);
    let mut needed: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
    for pack in &packs {
        let Some(manifest) = pack.manifest() else {
            continue;
        };
        // A manifest on a test path is a fixture's: its language demands nothing.
        let present = repo.manifests.iter().any(|path| {
            if path.file_name().is_none_or(|name| name != manifest) {
                return false;
            }
            let unit = repo.unit_of(path);
            let home = repo.units[unit].path.clone();
            !repo.is_test(unit, &home)
        });
        if !present {
            continue;
        }
        for binary in pack.axioms() {
            needed.entry(binary).or_default().push(pack.label());
        }
    }
    let mut missing = Vec::new();
    for (binary, labels) in needed {
        if answers(binary) {
            continue;
        }
        missing.push(format!(
            "{binary} is not on this machine, and {} units stand on it",
            labels.join(" and ")
        ));
    }
    missing
}

// Whether the binary exists and answers; --version is the one flag they all take.
fn answers(binary: &str) -> bool {
    Command::new(binary)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[cfg(test)]
mod tests {
    use super::answers;

    #[test]
    fn an_absent_binary_does_not_answer_and_a_present_one_does() {
        assert!(answers("true"));
        assert!(!answers("surely-no-machine-carries-this-binary"));
    }
}
