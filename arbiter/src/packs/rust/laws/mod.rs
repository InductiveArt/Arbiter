mod comment_overreach;
mod crowded_import;
mod hidden_name;
mod lowered_dial;
mod off_channel_dependency;
mod reexport;
mod tautology;
mod undeclared_dependency;
mod unset_dial;
mod unused_dependency;

use std::path::Path;

use crate::core::pack::Repo;
use crate::domain::finding::Finding;
use crate::packs::rust::language::MANIFEST;
use crate::packs::rust::manifest::declared_crates;
use crate::packs::rust::reading::Reading;

// Every law the Rust pack holds over its readings, in the order they are told.
pub fn instances(readings: &[Reading], root: &Path, repo: &Repo) -> Vec<Finding> {
    let mut findings = reexport::instances(readings);
    findings.extend(tautology::instances(readings, repo));
    findings.extend(crowded_import::instances(readings));
    findings.extend(hidden_name::instances(readings));
    findings.extend(comment_overreach::instances(readings));
    let declared = declared_crates(root, repo);
    findings.extend(undeclared_dependency::instances(readings, repo, &declared));
    findings.extend(unused_dependency::instances(
        readings, root, repo, &declared,
    ));
    findings
}

// The dials of one unit, read from the manifest in its folder.
pub fn dials(home: &Path, folder: &Path) -> Vec<Finding> {
    match std::fs::read_to_string(folder.join(MANIFEST)) {
        Ok(manifest) => manifest_dials(&manifest, home),
        Err(_) => vec![unset_dial::missing(home)],
    }
}

// The dials one manifest sets: every one unreadable when it does not parse,
// else each lint it lowers and each third party it takes off the channel.
fn manifest_dials(manifest: &str, home: &Path) -> Vec<Finding> {
    let Ok(parsed) = manifest.parse::<toml::Value>() else {
        return vec![unset_dial::unparsable(home)];
    };
    let mut findings = lowered_dial::instances(&parsed, home);
    findings.extend(off_channel_dependency::instances(&parsed, home));
    findings
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::manifest_dials;

    #[test]
    fn a_git_or_absolute_path_dependency_is_named() {
        let manifest = "[dependencies]\nserde = \"1\"\n\
                        forked = { git = \"https://github.com/x/forked\" }\n\
                        patched = { path = \"/abs/patched\" }\nlocal = { path = \"../local\" }\n";
        let findings = manifest_dials(manifest, Path::new("u"));
        assert!(findings.iter().any(|finding| finding
            .reason
            .starts_with("dependencies.forked comes from git;")));
        assert!(findings.iter().any(|finding| finding
            .reason
            .starts_with("dependencies.patched is an absolute path;")));
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn a_lint_turned_to_allow_is_named_in_either_form() {
        let manifest = "[lints.clippy]\ncmp_owned = \"allow\"\n\
                        [workspace.lints.rust]\ndead_code = { level = \"allow\", priority = 1 }\n";
        let findings = manifest_dials(manifest, Path::new("u"));
        assert!(findings.iter().any(|finding| finding
            .reason
            .starts_with("lints.clippy.cmp_owned is allow;")));
        assert!(findings.iter().any(|finding| finding
            .reason
            .starts_with("workspace.lints.rust.dead_code is allow;")));
    }

    #[test]
    fn a_lint_raised_to_deny_passes_the_dials() {
        let manifest = "[lints.clippy]\nunwrap_used = \"deny\"\n";
        assert!(manifest_dials(manifest, Path::new("u")).is_empty());
    }
}
