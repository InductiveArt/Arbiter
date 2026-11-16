mod comment_overreach;
mod compiler_options;
mod crowded_import;
mod entry;
mod hidden_name;
mod off_channel_dependency;
mod phantom_import;
mod reexport;
mod tautology;
mod undeclared_dependency;
mod undeclared_protocol;
mod unset_dial;
mod unused_dependency;

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::pack::Repo;
use crate::domain::finding::Finding;
use crate::packs::typescript::language::MANIFEST;
use crate::packs::typescript::packages::declared_packages;
use crate::packs::typescript::reading::Reading;
use crate::packs::typescript::routes::manifest_routes;
use crate::packs::typescript::surfaces::surface_definitions;

// The compiler, asked for the options a unit's tsconfig resolves to.
const RESOLVE: [&str; 8] = [
    "npx",
    "--yes",
    "--package",
    "typescript@7.0.2",
    "tsc",
    "--showConfig",
    "-p",
    ".",
];

// Every law the TypeScript pack holds over its readings, in the order they
// are told. A surface holding definitions is at once the offering file and
// the defining file, and every law telling the two roles apart misreads it.
pub fn instances(readings: &[Reading], root: &Path, repo: &Repo) -> Vec<Finding> {
    let present: HashSet<&PathBuf> = repo.files.iter().collect();
    let routes = manifest_routes(root, repo, &present);
    let defined = surface_definitions(root, &routes.surfaces);
    let mut findings = reexport::instances(readings, repo, &routes, &defined);
    findings.extend(hidden_name::instances(readings));
    findings.extend(tautology::instances(readings, repo));
    findings.extend(crowded_import::instances(readings));
    findings.extend(comment_overreach::instances(readings));
    let declared = declared_packages(root, repo);
    findings.extend(undeclared_dependency::instances(readings, repo, &declared));
    findings.extend(phantom_import::instances(readings, repo));
    findings.extend(unused_dependency::instances(
        readings, root, repo, &declared,
    ));
    findings.extend(undeclared_protocol::instances(readings, repo));
    findings.extend(entry::instances(
        readings, repo, &routes, &defined, &present,
    ));
    findings
}

// The dials of one unit: the compiler options its tsconfig resolves to, then
// its manifest's third parties; a note when the compiler gives no answer.
pub fn dials(home: &Path, folder: &Path) -> (Vec<Finding>, Vec<String>) {
    let finished = std::process::Command::new(RESOLVE[0])
        .args(&RESOLVE[1..])
        .current_dir(folder)
        .output();
    let Ok(output) = finished else {
        return (Vec::new(), vec![format!("{} did not start", RESOLVE[0])]);
    };
    if !output.status.success() {
        return (vec![unset_dial::missing(home)], Vec::new());
    }
    let Ok(resolved) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return (
            Vec::new(),
            vec!["tsc --showConfig answered something unreadable".to_string()],
        );
    };
    let options = resolved
        .get("compilerOptions")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let mut found = compiler_options::instances(&options, home);
    if let Ok(manifest) = std::fs::read_to_string(folder.join(MANIFEST)) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&manifest) {
            found.extend(off_channel_dependency::instances(&parsed, home));
        }
    }
    (found, Vec::new())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::compiler_options;
    use super::off_channel_dependency;

    #[test]
    fn a_dependency_outside_the_registry_or_workspace_is_named() {
        let manifest = serde_json::json!({
            "dependencies": {
                "lodash": "^4.17.21",
                "ui": "workspace:*",
                "forked": "github:x/forked",
                "grabbed": "https://example.com/pkg.tgz"
            },
            "devDependencies": { "linked": "link:../elsewhere" }
        });
        let findings = off_channel_dependency::instances(&manifest, Path::new("u"));
        assert!(findings.iter().any(|finding| finding
            .reason
            .starts_with("dependencies.forked is \"github:x/forked\";")));
        assert!(findings
            .iter()
            .any(|finding| finding.reason.contains("dependencies.grabbed")));
        assert!(findings
            .iter()
            .any(|finding| finding.reason.contains("devDependencies.linked")));
        assert_eq!(findings.len(), 3);
    }

    #[test]
    fn the_bar_names_each_unset_option_and_leaves_a_met_one() {
        let options = serde_json::json!({ "strict": true });
        let findings = compiler_options::instances(&options, Path::new("u"));
        assert!(findings
            .iter()
            .any(|finding| finding.reason.contains("noUnusedLocals is unset")));
        assert!(!findings
            .iter()
            .any(|finding| finding.reason.contains("strict is")));
    }

    #[test]
    fn a_lowered_option_is_named_with_its_value() {
        let options = serde_json::json!({ "allowUnreachableCode": true });
        assert!(compiler_options::instances(&options, Path::new("u"))
            .iter()
            .any(|finding| finding.reason
                == "compilerOptions.allowUnreachableCode is true; the bar says false"));
    }
}
