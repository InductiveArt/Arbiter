use std::fs;
use std::path::Path;

const UNITS_SEED: &str = "\
# The unit graph, declared. The gate holds the resolved imports to this file\n\
# one unit at a time: a unit absent here is unchecked, so adopt at your pace.\n\
#\n\
# [units.\"libs/example\"]\n\
# # Who may import this unit: exact paths, a folder and below as \"apps/*\", or \"*\".\n\
# visibility = [\"apps/*\"]\n\
# # The units this one uses, held both ways: using an undeclared unit is a\n\
# # finding, and so is declaring one never used.\n\
# deps = [\"libs/other\"]\n\
#\n\
# [units.\"libs/example\".inner.\"src/api\"]\n\
# # Inside the unit the same two consents apply to a folder or a file, and a\n\
# # child only tightens what the unit allows. `//` reaches from the repo root.\n\
# visibility = [\"src/app/*\"]\n\
# deps = [\"src/domain\", \"//libs/other\"]\n";

const SUPPRESSIONS_SEED: &str = "\
# Record suppressions. Each entry quiets one rule for the packages it names,\n\
# in the units it states, and is counted in the ledger like any suppression.\n\
# An entry that quiets nothing is stale; a form wider than its named subjects\n\
# is a blanket, refused and never recorded.\n\
#\n\
# [[suppression]]\n\
# rule = \"unused-dependency\"\n\
# language = \"python\"\n\
# packages = [\"coverage\"]\n\
# units = [\"*\"]\n\
#\n\
# [[suppression]]\n\
# rule = \"sibling-cycle\"\n\
# unit = \".\"\n\
# # The whole member set, exactly: a third member joining the cycle fires fresh.\n\
# members = [\"src/a\", \"src/b\"]\n";

const LICENSES_SEED: &str = "\
# The licenses third parties may carry. Until an allow list is written here,\n\
# no license policy exists; once written, every dependency whose license the\n\
# list does not name is a finding. An empty list allows nothing.\n\
#\n\
# allow = [\n\
#   \"MIT\",\n\
#   \"Apache-2.0\",\n\
# ]\n";

// The decision files, seeded as comments the first time the gate runs: an
// invitation that declares nothing, never overwritten once present.
pub fn seeded(root: &Path) {
    let folder = root.join(".arbiter");
    if fs::create_dir_all(&folder).is_err() {
        return;
    }
    for (name, seed) in [
        ("units.toml", UNITS_SEED),
        ("licenses.toml", LICENSES_SEED),
        ("suppressions.toml", SUPPRESSIONS_SEED),
    ] {
        let path = folder.join(name);
        if !path.exists() {
            let _ = fs::write(path, seed);
        }
    }
}
