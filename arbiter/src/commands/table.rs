// The whole grammar: each command with its one line. The bare call prints it,
// and a test holds the skill to these exact sentences.
pub const COMMANDS: &[(&str, &str)] = &[
    (
        "gate [--fix] [--cold]",
        "judge everything; red names what stands between the code and clean",
    ),
    (
        "find [rule]",
        "every tool-free finding, as JSON lines, in seconds; a rule names one law's instances",
    ),
    (
        "find fallout <name>",
        "every definition of a name and each resolved use",
    ),
    (
        "find deps [unit|path]",
        "what one unit stands on, declared against resolved; a folder or file at its own grain; bare, the whole graph as mermaid",
    ),
    (
        "find rhymes",
        "structurally equal functions, largest first; a measure, never findings",
    ),
    (
        "find tangles [unit|path]",
        "entangled siblings per folder, largest first; a measure, never findings",
    ),
    (
        "ledger",
        "the signature's account: suppressions and third parties against the reference",
    ),
    (
        "ledger sign",
        "record today's account as the reference: the user's signature",
    ),
    (
        "rules [rule]",
        "the matrix in groups; a rule prints its law and whys, a group its laws",
    ),
    (
        "adopt",
        "install the agent layer; its files refresh with the binary, never backwards",
    ),
];

// The bare call teaches: the whole grammar, each command with its one line.
pub fn taught() {
    println!(
        "arbiter {} judges a repository: the resolved graph and each unit's pinned tools.",
        crate::core::VERSION
    );
    println!();
    let width = COMMANDS
        .iter()
        .map(|(call, _)| call.len())
        .max()
        .unwrap_or(0);
    for (call, line) in COMMANDS {
        println!("  {call:width$}  {line}");
    }
    println!();
    println!("gate, adopt and ledger sign act; find, ledger and rules read and never write.");
    println!(
        "A command takes the folder last; missing, it judges here. \
         Exit: 0 clean, 1 findings, 2 no judgment — a broken machine, \
         a bad ask, or nothing to judge."
    );
}
