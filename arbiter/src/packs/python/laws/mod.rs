mod comment_overreach;
mod hidden_name;
mod lowered_dial;
mod off_channel_dependency;
mod phantom_import;
mod reexport;
mod tautology;
mod undeclared_dependency;
mod undeclared_protocol;
mod unset_dial;
mod unused_dependency;

use std::fs;
use std::path::Path;

use crate::core::pack::Repo;
use crate::domain::finding::Finding;
use crate::packs::python::distributions::declared_distributions;
use crate::packs::python::language::MANIFEST;
use crate::packs::python::reading::Reading;

// Every law the Python pack holds over its readings, in the order they are told.
pub fn instances(readings: &[Reading], root: &Path, repo: &Repo) -> Vec<Finding> {
    let mut findings = reexport::instances(readings);
    findings.extend(hidden_name::instances(readings));
    findings.extend(tautology::instances(readings, repo));
    findings.extend(comment_overreach::instances(readings));
    let declared = declared_distributions(root, repo);
    findings.extend(undeclared_dependency::instances(readings, repo, &declared));
    findings.extend(phantom_import::instances(readings, repo));
    findings.extend(unused_dependency::instances(
        readings, root, repo, &declared,
    ));
    findings.extend(undeclared_protocol::instances(readings, repo));
    findings
}

// The dials of one unit, read from the manifest in its folder.
pub fn dials(home: &Path, folder: &Path) -> Vec<Finding> {
    match fs::read_to_string(folder.join(MANIFEST)) {
        Ok(manifest) => manifest_dials(&manifest, home),
        Err(_) => vec![unset_dial::missing(home)],
    }
}

// The dials one manifest sets: every one unreadable when it does not parse,
// else each setting lowering the bar and each third party off the channel.
fn manifest_dials(manifest: &str, home: &Path) -> Vec<Finding> {
    let Ok(parsed) = manifest.parse::<toml::Value>() else {
        return vec![unset_dial::unparsable(home)];
    };
    let mut findings = lowered_dial::instances(&parsed, home);
    findings.extend(off_channel_dependency::instances(&parsed, home));
    findings
}

// The value at a dotted path of a manifest, when every step of it is there.
fn lookup<'manifest>(parsed: &'manifest toml::Value, path: &str) -> Option<&'manifest toml::Value> {
    let mut value = parsed;
    for part in path.split('.') {
        value = value.get(part)?;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::manifest_dials;

    #[test]
    fn an_exclude_may_name_generated_code_and_nothing_else() {
        let manifest =
            "[tool.basedpyright]\nexclude = [\"**/*_pb2.py\", \"**/*_pb2.pyi\", \"src/core\"]\n";
        let findings = manifest_dials(manifest, Path::new("u"));
        assert_eq!(findings.len(), 1);
        assert!(findings[0]
            .reason
            .starts_with("tool.basedpyright.exclude hides src/core;"));
    }

    #[test]
    fn a_dependency_pulled_past_the_registry_is_named() {
        let manifest = "[project]\nname = \"a\"\nversion = \"0\"\n\
                        dependencies = [\"requests>=2\", \"left @ git+https://github.com/x/left\"]\n\
                        [dependency-groups]\ndev = [\"tool @ https://example.com/tool.whl\"]\n\
                        [tool.uv.sources]\nforked = { git = \"https://github.com/x/forked\" }\n\
                        patched = { path = \"/abs/patched\" }\nlocal = { path = \"../local\" }\n";
        let findings = manifest_dials(manifest, Path::new("u"));
        assert!(findings.iter().any(|finding| {
            finding
                .reason
                .starts_with("project.dependencies pulls left past the registry;")
        }));
        assert!(findings.iter().any(|finding| {
            finding
                .reason
                .starts_with("dependency-groups.dev pulls tool past the registry;")
        }));
        assert!(findings.iter().any(|finding| finding
            .reason
            .starts_with("tool.uv.sources.forked points past the registry;")));
        assert!(findings.iter().any(|finding| finding
            .reason
            .starts_with("tool.uv.sources.patched is an absolute path;")));
        assert_eq!(findings.len(), 4);
    }

    #[test]
    fn a_setting_lowering_the_bar_is_named_with_its_reason() {
        let manifest = "[tool.ruff.lint]\nignore = [\"E501\"]\n";
        let findings = manifest_dials(manifest, Path::new("u"));
        assert!(findings
            .iter()
            .any(|finding| finding.reason.starts_with("tool.ruff.lint.ignore is set;")));
    }

    #[test]
    fn a_lowered_checker_rule_and_mode_are_named() {
        let manifest =
            "[tool.basedpyright]\ntypeCheckingMode = \"basic\"\nreportAny = \"warning\"\n";
        let findings = manifest_dials(manifest, Path::new("u"));
        assert!(findings
            .iter()
            .any(|finding| finding.reason.contains("typeCheckingMode is basic")));
        assert!(findings
            .iter()
            .any(|finding| finding.reason.contains("reportAny is lowered")));
    }

    #[test]
    fn an_environment_setting_passes_the_dials() {
        let manifest = "[project]\nname = \"a\"\nversion = \"0\"\n\
                        [tool.ruff]\nextend = \"shared.toml\"\n\
                        [tool.basedpyright]\nextends = \"shared.json\"\n";
        assert!(manifest_dials(manifest, Path::new("u")).is_empty());
    }
}
