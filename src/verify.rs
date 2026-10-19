use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::pack::Pack;
use crate::units::Repo;

pub struct Outcome {
    pub unit: String,
    pub pack: &'static str,
    pub check: String,
    pub passed: bool,
    pub seconds: f64,
    pub tail: String,
}

const TAIL: usize = 15;

// Runs every declared adapter over every unit holding its manifest. A unit of a
// language whose pack declares no tools is named in the notes rather than passed over.
pub fn verify(root: &Path, repo: &Repo, packs: &[Box<dyn Pack>]) -> (Vec<Outcome>, Vec<String>) {
    let mut outcomes = Vec::new();
    let mut notes = Vec::new();
    for (index, unit) in repo.units.iter().enumerate() {
        // A unit on a test path is a fixture: data for some test, not code to judge.
        if repo.is_test(index, &unit.path) {
            continue;
        }
        let folder = root.join(&unit.path);
        let shown = if unit.path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            unit.path.to_string_lossy().to_string()
        };
        for pack in packs {
            let Some(manifest) = pack.manifest() else {
                continue;
            };
            if !folder.join(manifest).is_file() {
                continue;
            }
            let Some(adapter) = pack.adapter() else {
                notes.push(format!(
                    "{shown} is {}, which declares no tools yet",
                    pack.label()
                ));
                continue;
            };
            for check in checks(adapter) {
                outcomes.push(run(&shown, pack.label(), &folder, &check));
            }
        }
    }
    (outcomes, notes)
}

struct Check {
    name: String,
    run: Vec<String>,
}

fn checks(adapter: &str) -> Vec<Check> {
    let Ok(parsed) = adapter.parse::<toml::Value>() else {
        return Vec::new();
    };
    let Some(entries) = parsed.get("check").and_then(toml::Value::as_array) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?.to_string();
            let run = entry
                .get("run")?
                .as_array()?
                .iter()
                .filter_map(|word| word.as_str().map(str::to_string))
                .collect::<Vec<String>>();
            (!run.is_empty()).then_some(Check { name, run })
        })
        .collect()
}

fn run(unit: &str, pack: &'static str, folder: &Path, check: &Check) -> Outcome {
    let started = Instant::now();
    let finished = Command::new(&check.run[0])
        .args(&check.run[1..])
        .current_dir(folder)
        .output();
    let seconds = started.elapsed().as_secs_f64();
    let (passed, tail) = match finished {
        Ok(output) => (output.status.success(), last_lines(&output)),
        Err(error) => (false, format!("{} did not start: {error}", check.run[0])),
    };
    Outcome {
        unit: unit.to_string(),
        pack,
        check: check.name.clone(),
        passed,
        seconds,
        tail: if passed { String::new() } else { tail },
    }
}

fn last_lines(output: &std::process::Output) -> String {
    let whole = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<&str> = whole.lines().collect();
    let kept = &lines[lines.len().saturating_sub(TAIL)..];
    kept.join("\n")
}
