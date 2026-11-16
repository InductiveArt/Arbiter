use crate::core::edge::Edge;
use crate::core::graph::Graph;
use crate::core::rule;
use crate::core::rule::Law;
use crate::core::rule::Rule;
use crate::domain::finding::line_at;
use crate::domain::finding::Finding;

pub const RULES: &[Rule] = &[
    rule::facts(
        "dead-symbol",
        "symbols",
        "a public symbol no user reaches is named for deletion",
    ),
    rule::facts(
        "tests-only-symbol",
        "symbols",
        "a test keeps nothing alive: a symbol only tests reach is scaffolding or dead",
    ),
];

pub fn economy_law() -> Law {
    Law {
        task: "economy",
        rules: RULES,
        run: winnow,
    }
}

// A public symbol no file but its own uses is named, inviting deletion.
pub fn winnow(found: &Graph) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (index, def) in found.defs.iter().enumerate() {
        if def.self_used {
            continue;
        }
        // A PUBLISHED unit's surface serves users the repository cannot see.
        if found.repo.is_published(def.unit) {
            continue;
        }
        let users: Vec<&Edge> = found
            .edges
            .iter()
            .filter(|edge| edge.def == index && edge.file != def.file)
            .collect();
        // A test keeps nothing alive, wherever it lives.
        if users
            .iter()
            .any(|edge| !found.repo.is_test(edge.unit, &edge.file))
        {
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
            check: "winnow".to_string(),
            file: def.file.clone(),
            line: line_at(def.line),
            rule: Some(name.to_string()),
            reason,
        });
    }
    findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    findings
}
