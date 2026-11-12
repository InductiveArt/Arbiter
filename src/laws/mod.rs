pub mod findings;

use crate::core::graph::Graph;
use crate::core::pack::Finding;
use crate::core::task::Law;

// Every graph law the gate runs under its judge task, one entry each. The record
// laws live with their records and the per-language laws with their packs; all
// three run beside each other under judge.
pub fn all() -> Vec<Law> {
    vec![
        Law {
            task: "cycles",
            run: cycles,
        },
        Law {
            task: "boundaries",
            run: boundaries,
        },
        Law {
            task: "folders",
            run: folders,
        },
        Law {
            task: "licenses",
            run: licenses,
        },
        Law {
            task: "versions",
            run: versions,
        },
        Law {
            task: "suppressions",
            run: suppressions,
        },
    ]
}

// Every tool-free finding, flat: the graph laws, the record laws and the
// per-language laws, the form find and the tests read. The gate runs the same
// laws one task each.
pub fn judged(found: &Graph) -> Vec<Finding> {
    let mut gathered = Vec::new();
    for law in all() {
        gathered.extend((law.run)(found));
    }
    gathered.extend(crate::records::plans::findings(found));
    gathered.extend(crate::records::brief::findings(found));
    for pack in &found.packs {
        gathered.extend(pack.integrity(&found.root, &found.repo));
    }
    gathered.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    gathered
}

fn cycles(found: &Graph) -> Vec<Finding> {
    findings::cycles_law(found)
}

fn boundaries(found: &Graph) -> Vec<Finding> {
    findings::boundaries(found)
}

fn folders(found: &Graph) -> Vec<Finding> {
    findings::folder_boundaries(found)
}

fn licenses(found: &Graph) -> Vec<Finding> {
    findings::licenses(found)
}

fn versions(found: &Graph) -> Vec<Finding> {
    findings::versions_law(found)
}

fn suppressions(found: &Graph) -> Vec<Finding> {
    findings::suppressions_law(found)
}
