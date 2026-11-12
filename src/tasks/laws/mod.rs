pub mod boundaries;
pub mod cycles;
pub mod economy;
pub mod folders;
pub mod licenses;
pub mod suppressions;
pub mod versions;

use crate::core::graph::Graph;
use crate::core::pack::Finding;
use crate::core::rule;
use crate::core::rule::Law;
use crate::core::rule::Power;
use crate::core::rule::Rule;
use crate::records::brief::brief_law;
use crate::records::plans::plans_law;
use boundaries::boundaries_law;
use cycles::cycles_law;
use folders::folders_law;
use licenses::licenses_law;
use suppressions::suppressions_law;
use versions::versions_law;

// The rules the economy law fires: declared here because that task yields notes
// as well as findings, so it runs beside the registry rather than inside it.
const ECONOMY: &[Rule] = &[
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

// The rules no single graph law owns: the vanished claim the claims task fires,
// and the unparsable record any record law fires when a record will not parse.
const VANISHED: &[Rule] = &[rule::record(
    "vanished-claim",
    "claims",
    ".arbiter/reference.json",
    "a claim once signed leaves only with the user's signature",
)];
const SHARED: &[Rule] = &[rule::facts(
    "unparsable-user-record",
    "claims",
    "a record the user wrote parses, or the break itself is the finding; a store is rebuilt, never judged",
)];

// Every graph and record law the gate runs under its judge task, one entry
// each; the per-language laws live with their packs and run beside these.
pub fn all() -> Vec<Law> {
    vec![
        cycles_law(),
        boundaries_law(),
        folders_law(),
        licenses_law(),
        versions_law(),
        suppressions_law(),
        plans_law(),
        brief_law(),
    ]
}

// Every tool-free finding, flat: the graph and record laws and the per-language
// laws, the form find and the tests read. The gate runs the same laws one task
// each.
pub fn judged(found: &Graph) -> Vec<Finding> {
    let mut gathered = Vec::new();
    for law in all() {
        gathered.extend((law.run)(found));
    }
    for pack in &found.packs {
        gathered.extend(pack.integrity(&found.root, &found.repo));
    }
    gathered.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    gathered
}

// Every rule the matrix holds: each law's own rules, the ones no single law
// owns, and the per-language vocabulary the packs hold. The grid derives from
// this instead of a list kept by hand beside the laws.
pub fn matrix() -> Vec<&'static Rule> {
    let mut rules: Vec<&Rule> = Vec::new();
    for law in all() {
        rules.extend(law.rules.iter());
    }
    rules.extend(ECONOMY.iter());
    rules.extend(VANISHED.iter());
    rules.extend(SHARED.iter());
    rules.extend(rule::PER_LANGUAGE.iter());
    rules
}

fn named(name: &str) -> Option<&'static Rule> {
    matrix().into_iter().find(|rule| rule.name == name)
}

// The law a name states, wherever its rule is declared.
pub fn law(name: &str) -> Option<&'static str> {
    named(name).map(|rule| rule.law)
}

// Where the named rule detains its power.
pub fn power(name: &str) -> Power {
    named(name).map_or(Power::Facts, |rule| rule.power)
}

// Whether arbiter rules answers the name: every finding carries one it does.
pub fn drillable(name: &str) -> bool {
    named(name).is_some()
}

// Whether tool-free findings can carry the name: a rule outside the gated set.
pub fn findable(name: &str) -> bool {
    named(name).is_some_and(|rule| !rule.gated)
}

// Whether the rule is one a pack holds or declines per language, rather than one
// the core judges once over the graph.
pub fn per_language(name: &str) -> bool {
    rule::PER_LANGUAGE.iter().any(|rule| rule.name == name)
}

// The rules by group, in the matrix's declared order, each group with its rules.
pub fn groups() -> Vec<(&'static str, Vec<&'static Rule>)> {
    let all = matrix();
    rule::GROUP_ORDER
        .iter()
        .map(|&group| {
            let held: Vec<&Rule> = all
                .iter()
                .filter(|rule| rule.group == group)
                .copied()
                .collect();
            (group, held)
        })
        .filter(|(_, held)| !held.is_empty())
        .collect()
}
