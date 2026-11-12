// A rule is a finding-type with its law: the self-describing vocabulary the
// matrix is built from. A judgment task declares the rules it fires, so the
// grid derives from the tasks instead of a hand-kept list beside them.
pub struct Rule {
    pub name: &'static str,
    pub law: &'static str,
    pub group: &'static str,
    pub power: Power,
    // Whether the rule fires through the gate's tools rather than as a tool-free
    // finding: a gated rule drills but is never found.
    pub gated: bool,
}

// Where a rule detains its power: the facts alone, or a record the user wrote -
// a record-powered rule waits until its record exists.
#[derive(Clone, Copy)]
pub enum Power {
    Facts,
    Record(&'static str),
}

// A judgment task: the rules it can fire, and the run that fires them over the
// resolved graph. The gate runs each law under its judge task; the matrix reads
// every law's rules to build the grid.
pub struct Law {
    pub task: &'static str,
    pub rules: &'static [Rule],
    pub run: fn(&crate::core::graph::Graph) -> Vec<crate::core::pack::Finding>,
}

// The matrix prints its groups in this order; a rule declares which group it
// sits in, and this fixes where that group's shelf appears.
pub const GROUP_ORDER: &[&str] = &[
    "symbols",
    "imports",
    "protocols",
    "plans",
    "folders",
    "units",
    "third-parties",
    "tooling",
    "suppressions",
    "claims",
    "generated",
    "comments",
    "images",
];

pub const fn facts(name: &'static str, group: &'static str, law: &'static str) -> Rule {
    Rule {
        name,
        law,
        group,
        power: Power::Facts,
        gated: false,
    }
}

pub const fn gated(name: &'static str, group: &'static str, law: &'static str) -> Rule {
    Rule {
        name,
        law,
        group,
        power: Power::Facts,
        gated: true,
    }
}

// A rule the user's record carries, named with the file that holds it.
pub const fn record(
    name: &'static str,
    group: &'static str,
    file: &'static str,
    law: &'static str,
) -> Rule {
    Rule {
        name,
        law,
        group,
        power: Power::Record(file),
        gated: false,
    }
}

// The rules a pack may hold or decline, the same vocabulary for every language:
// a pack teaches the core to read its ecosystem and says, per rule, whether it
// holds. The per-language stance lives in each pack's rules(); the law is here.
pub const PER_LANGUAGE: &[Rule] = &[
    facts("reexport", "imports", "a module offers only its own names; a surface names each symbol it offers and defines none"),
    facts("self-entry", "imports", "a unit's own code imports from the defining file"),
    facts("interior-import", "imports", "a unit is entered through the routes its manifest declares"),
    facts("hidden-name", "imports", "a symbol keeps its own name"),
    facts("crowded-import", "imports", "an import line names one symbol and the module it comes from"),
    facts("phantom-import", "imports", "a generated import resolves to the contract it was generated from; one naming no contract is a finding"),
    facts("comment-overreach", "comments", "a comment names only what its file defines, uses or quotes"),
    facts("undeclared-dependency", "third-parties", "every third party an import names is declared by the importing unit's manifest; a development table carries only what tests import"),
    facts("unused-dependency", "third-parties", "a third party a manifest declares and nothing reaches is named for deletion"),
    facts("undeclared-protocol", "protocols", "a class standing where a protocol is asked names it among its bases"),
    facts("unpinned-image", "images", "a container image is named by digest, never by a tag alone"),
    facts("stray-test", "claims", "every test names itself with a literal sentence; one built at run time cannot be heard"),
    facts("tautology", "claims", "an assertion compares the system to an expectation; a value equal to itself asserts nothing"),
    Rule {
        name: "stale-suppression",
        law: "a recorded suppression quiets one finding this run, or is named: one that suppresses nothing is stale",
        group: "suppressions",
        power: Power::Record(".arbiter/suppressions.toml"),
        gated: false,
    },
    gated("unset-dial", "tooling", "a dial the bar requires is present in the unit's config"),
    gated("lowered-dial", "tooling", "a dial sits at or above the pack's floor; a setting may add, never lower"),
    gated("untested-branch", "tooling", "a unit's tests reach every branch of its source"),
    gated("off-channel-dependency", "third-parties", "a dependency comes from the locked registry or this repository; git, url and absolute paths slip the lock"),
    gated("stale-generated", "generated", "generated code matches what its contracts generate; missing, differing and orphaned output is named"),
    gated("inter-language-break", "generated", "an inter-language contract breaks its signed image only with the user's signature"),
];
