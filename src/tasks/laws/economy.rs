use crate::core::pack::Def;
use crate::core::pack::Edge;
use crate::core::pack::Finding;
use crate::core::pack::Repo;
use std::path::PathBuf;

// A public symbol no file but its own uses is named, inviting deletion.
pub fn winnow(
    defs: &[Def],
    edges: &[Edge],
    repo: &Repo,
    commissioned: &std::collections::HashSet<PathBuf>,
) -> (Vec<Finding>, Vec<String>) {
    let mut findings = Vec::new();
    let mut nascent = Vec::new();
    for (index, def) in defs.iter().enumerate() {
        if def.self_used {
            continue;
        }
        // A PUBLISHED unit's surface serves users the repository cannot see.
        if repo.is_published(def.unit) {
            continue;
        }
        let users: Vec<&Edge> = edges
            .iter()
            .filter(|edge| edge.def == index && edge.file != def.file)
            .collect();
        // A test keeps nothing alive, wherever it lives.
        if users
            .iter()
            .any(|edge| !repo.is_test(edge.unit, &edge.file))
        {
            continue;
        }
        // Tests of a case the brief still plans are building toward a consumer:
        // the symbol is nascent, and the commission is the user's signature.
        if !users.is_empty() && users.iter().all(|edge| commissioned.contains(&edge.file)) {
            nascent.push(format!(
                "nascent: {} at {}:{} is reached only by tests of a case still planned",
                def.name,
                def.file.to_string_lossy(),
                def.line
            ));
            continue;
        }
        let (name, reason) = if users.is_empty() {
            ("dead-symbol", format!("{} has no user anywhere", def.name))
        } else {
            (
                "tests-only-symbol",
                format!("{} has no user but tests", def.name),
            )
        };
        findings.push(Finding {
            check: "winnow",
            file: def.file.clone(),
            line: def.line,
            name: name.to_string(),
            reason,
        });
    }
    findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    nascent.sort();
    (findings, nascent)
}
