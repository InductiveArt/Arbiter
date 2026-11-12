use std::process::ExitCode;

use crate::core::rule::Power;
use crate::laws;

pub fn run(arguments: &[String]) -> ExitCode {
    if arguments.first().map(String::as_str) == Some("rules") {
        return match arguments.get(1) {
            None => {
                matrix();
                ExitCode::SUCCESS
            }
            Some(rule) => ruled(rule),
        };
    }
    // Dispatched only for this command, so the guard always holds.
    ExitCode::from(2)
}

// The matrix of rule against language, printed group by group. A cell answers
// held or never; an open cell is work the matrix keeps visible, never a pass.
// The tag column answers origin and power at a glance: a core row is judged
// on one graph, a record row waits for the user's record.
fn matrix() {
    let packs = crate::packs::all();
    let width = laws::matrix()
        .iter()
        .map(|rule| rule.name.len())
        .max()
        .unwrap_or(0);
    print!("  {:width$}  {:14}", "rule", "power");
    for pack in &packs {
        print!("  {:10}", pack.label());
    }
    println!();
    for (group, members) in laws::groups() {
        println!("{group}");
        for rule in members {
            if !laws::per_language(rule.name) {
                println!("  {:width$}  {}", rule.name, tagged(rule.name));
                continue;
            }
            print!("  {:width$}  {:14}", rule.name, tagged(rule.name));
            for pack in &packs {
                let answer = pack
                    .rules()
                    .iter()
                    .find(|(name, _)| *name == rule.name)
                    .map(|(_, answer)| *answer);
                let cell = match answer {
                    Some("held") => "held",
                    Some(_) => "never",
                    None => "open",
                };
                print!("  {cell:10}");
            }
            println!();
        }
    }
    println!();
    println!("core is judged on one graph: every unit and import, all languages together.");
    println!(
        "facts rules are the bar from day one; a record rule waits for the user's \
         record, and the drill names its file."
    );
}

// The glance column: origin and power in one word each.
fn tagged(rule: &str) -> String {
    let core = !laws::per_language(rule);
    let record = matches!(laws::power(rule), Power::Record(_));
    match (core, record) {
        (true, true) => "core, record".to_string(),
        (true, false) => "core, facts".to_string(),
        (false, true) => "record".to_string(),
        (false, false) => "facts".to_string(),
    }
}

// One rule whole: its law, then each pack's stance - held, open, or never
// with its why. A core judgment has no stances; its law is the answer. A
// group prints every law on its shelf.
fn ruled(asked: &str) -> ExitCode {
    for (group, members) in laws::groups() {
        if group != asked {
            continue;
        }
        println!("{group}:");
        for rule in members {
            println!("  {}: {}", rule.name, rule.law);
        }
        return ExitCode::SUCCESS;
    }
    if let Some(law) = laws::law(asked) {
        println!("{asked}");
        labeled("law", law);
        labeled("power", &empowered(asked));
        if laws::per_language(asked) {
            labeled("judged", "by each language's pack, stance by stance");
            println!();
            for pack in crate::packs::all() {
                let answer = pack
                    .rules()
                    .iter()
                    .find(|(name, _)| *name == asked)
                    .map(|(_, answer)| *answer);
                match answer {
                    Some("held") => println!("  {:10} held", pack.label()),
                    Some(told) => println!(
                        "  {:10} never — {}",
                        pack.label(),
                        told.strip_prefix("never: ").unwrap_or(told)
                    ),
                    None => println!("  {:10} open", pack.label()),
                }
            }
        } else {
            labeled(
                "judged",
                "on the one graph — every unit and import, all languages at once",
            );
        }
        return ExitCode::SUCCESS;
    }
    eprintln!("rules: no rule or group named {asked}; arbiter rules prints the matrix");
    ExitCode::from(2)
}

// Where the law detains its power: the facts alone, or a record the user
// wrote - a record-powered law waits until its record exists.
fn empowered(rule: &str) -> String {
    match laws::power(rule) {
        Power::Facts => "the facts".to_string(),
        Power::Record(file) => {
            format!("the user's record ({file}); without the record, the law waits")
        }
    }
}

// One labeled line of the drill, wrapped at a readable width with a hanging
// indent so the label column stays a column.
fn labeled(label: &str, text: &str) {
    let width = 68;
    let mut line = String::new();
    let mut first = true;
    for word in text.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > width {
            if first {
                println!("  {label:7}{line}");
                first = false;
            } else {
                println!("  {:7}{line}", "");
            }
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if first {
        println!("  {label:7}{line}");
    } else if !line.is_empty() {
        println!("  {:7}{line}", "");
    }
}
