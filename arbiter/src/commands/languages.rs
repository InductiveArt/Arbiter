use std::process::ExitCode;

use crate::core::pack::Pack;
use crate::core::tools::checks;
use crate::packs::all;
use crate::wip_domain::ecosystem::Ecosystem;
use crate::wip_tasks::load_ecosystems::load_ecosystems;

const HEADS: [&str; 7] = [
    "language",
    "unit marker",
    "members only when",
    "checks",
    "needs",
    "lockfiles",
    "never read",
];

pub fn run() -> ExitCode {
    match load_ecosystems() {
        Ok(ecosystems) => {
            print!("{}", shown(&all(), &ecosystems));
            ExitCode::SUCCESS
        }
        Err(failure) => {
            eprintln!("{}", failure.reason);
            ExitCode::from(2)
        }
    }
}

// Each language as it is declared. Its ecosystem record gives the file
// marking a unit and when that file only lists members; its pack gives the
// checks holding a unit in the order run, the executables those stand on,
// the lockfiles read and the folders never read.
pub fn shown(packs: &[Box<dyn Pack>], ecosystems: &[Ecosystem]) -> String {
    let mut rows = vec![HEADS.map(str::to_string)];
    for pack in packs {
        let ecosystem = ecosystems
            .iter()
            .find(|ecosystem| ecosystem.name == pack.label());
        let held: Vec<String> = pack
            .adapter()
            .map(checks)
            .unwrap_or_default()
            .into_iter()
            .map(|check| check.name)
            .collect();
        let held: Vec<&str> = held.iter().map(String::as_str).collect();
        rows.push([
            pack.label().to_string(),
            ecosystem
                .map_or("none", |ecosystem| ecosystem.marker.as_str())
                .to_string(),
            ruled(ecosystem),
            joined(&held),
            joined(pack.axioms()),
            joined(pack.lockfiles()),
            joined(pack.outputs()),
        ]);
    }
    let widths: [usize; 7] =
        std::array::from_fn(|column| rows.iter().map(|row| row[column].len()).max().unwrap_or(0));
    let mut out = String::new();
    for row in &rows {
        let cells: Vec<String> = row
            .iter()
            .zip(widths)
            .map(|(cell, width)| format!("{cell:width$}"))
            .collect();
        out.push_str(cells.join("  ").trim_end());
        out.push('\n');
    }
    out
}

fn ruled(ecosystem: Option<&Ecosystem>) -> String {
    match ecosystem.and_then(|ecosystem| ecosystem.members_only.as_ref()) {
        None => "never".to_string(),
        Some(rule) if rule.lacks.is_empty() => format!("has {}", rule.has.join(", ")),
        Some(rule) => format!(
            "has {}, lacks {}",
            rule.has.join(", "),
            rule.lacks.join(", ")
        ),
    }
}

fn joined(names: &[&str]) -> String {
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    }
}
