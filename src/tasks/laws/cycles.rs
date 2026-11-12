use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::core::graph::strongly_connected;
use crate::core::graph::Graph;
use crate::core::pack::Def;
use crate::core::pack::Edge;
use crate::core::pack::Finding;
use crate::core::pack::Repo;
use crate::core::rule;
use crate::core::rule::Law;
use crate::core::rule::Rule;

pub const RULES: &[Rule] = &[rule::facts(
    "unit-cycle",
    "units",
    "units form an order; no two import each other, however long the loop",
)];

pub fn cycles_law() -> Law {
    Law {
        task: "cycles",
        rules: RULES,
        run: judge,
    }
}

// The unit cycles, as one law reads them from the resolved graph.
fn judge(found: &Graph) -> Vec<Finding> {
    cycles(&found.defs, &found.edges, &found.repo)
}

fn cycles(defs: &[Def], edges: &[Edge], repo: &Repo) -> Vec<Finding> {
    let mut adjacency = vec![BTreeSet::new(); repo.units.len()];
    for edge in edges {
        let owner = defs[edge.def].unit;
        if owner != edge.unit {
            adjacency[edge.unit].insert(owner);
        }
    }
    let mut findings = Vec::new();
    for group in strongly_connected(&adjacency) {
        if group.len() < 2 {
            continue;
        }
        let mut names: Vec<String> = group
            .iter()
            .map(|&unit| repo.units[unit].path.to_string_lossy().to_string())
            .collect();
        names.sort();
        findings.push(Finding {
            check: "integrity",
            file: PathBuf::from(&names[0]),
            line: 0,
            name: "unit-cycle".to_string(),
            reason: format!("units import each other: {}", names.join(", ")),
        });
    }
    findings
}
