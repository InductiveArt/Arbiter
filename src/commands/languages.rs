use std::process::ExitCode;

use crate::core::pack::Pack;
use crate::core::tools::checks;
use crate::packs::all;

const HEADS: [&str; 6] = [
    "language",
    "unit marker",
    "checks",
    "needs",
    "lockfiles",
    "never read",
];

pub fn run() -> ExitCode {
    print!("{}", shown(&all()));
    ExitCode::SUCCESS
}

// Each language as its pack declares it: the file marking a unit, the checks
// holding one in the order run, the executables those stand on, the
// lockfiles read and the folders never read.
pub fn shown(packs: &[Box<dyn Pack>]) -> String {
    let mut rows = vec![HEADS.map(str::to_string)];
    for pack in packs {
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
            pack.manifest().unwrap_or("none").to_string(),
            joined(&held),
            joined(pack.axioms()),
            joined(pack.lockfiles()),
            joined(pack.outputs()),
        ]);
    }
    let widths: [usize; 6] =
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

fn joined(names: &[&str]) -> String {
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    }
}
