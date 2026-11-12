pub mod boundaries;
pub mod cycles;
pub mod economy;
pub mod folders;
pub mod licenses;
pub mod suppressions;
pub mod versions;

use crate::core::graph::Graph;
use crate::core::pack::Finding;
use crate::core::task::Law;
use boundaries::boundaries_law;
use cycles::cycles_law;
use folders::folders_law;
use licenses::licenses_law;
use suppressions::suppressions_law;
use versions::versions_law;

// Every graph law the gate runs under its judge task, one file each. The record
// laws live with their records and the per-language laws with their packs; all
// three run beside each other under judge.
pub fn all() -> Vec<Law> {
    vec![
        cycles_law(),
        boundaries_law(),
        folders_law(),
        licenses_law(),
        versions_law(),
        suppressions_law(),
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
