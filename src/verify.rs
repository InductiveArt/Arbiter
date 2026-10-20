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
    judged(root, repo, packs, false)
}

// Runs each check's declared fix first, then judges as verify does. A fix that fails
// is the outcome; one that succeeds leaves only the check's own verdict to show.
pub fn fix(root: &Path, repo: &Repo, packs: &[Box<dyn Pack>]) -> (Vec<Outcome>, Vec<String>) {
    judged(root, repo, packs, true)
}

fn judged(
    root: &Path,
    repo: &Repo,
    packs: &[Box<dyn Pack>],
    fixing: bool,
) -> (Vec<Outcome>, Vec<String>) {
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
            let Ok(declared) = std::fs::read_to_string(folder.join(manifest)) else {
                continue;
            };
            if pack.container(&declared) {
                notes.push(format!(
                    "{shown} is a {} workspace; its members are the units judged",
                    pack.label()
                ));
                continue;
            }
            let started = Instant::now();
            if let Some(findings) = pack.wiring(&folder) {
                outcomes.push(Outcome {
                    unit: shown.clone(),
                    pack: pack.label(),
                    check: "wiring".to_string(),
                    passed: findings.is_empty(),
                    seconds: started.elapsed().as_secs_f64(),
                    tail: findings.join("\n"),
                });
            }
            let Some(adapter) = pack.adapter() else {
                notes.push(format!(
                    "{shown} is {}, which declares no tools yet",
                    pack.label()
                ));
                continue;
            };
            let home = materialized(pack.as_ref());
            for check in checks(adapter) {
                let check = Check {
                    name: check.name,
                    run: filled(&check.run, home.as_deref()),
                    fix: check.fix.map(|fix| filled(&fix, home.as_deref())),
                };
                if fixing {
                    if let Some(fix) = &check.fix {
                        let repair = Check {
                            name: check.name.clone(),
                            run: fix.clone(),
                            fix: None,
                        };
                        let outcome = run(&shown, pack.label(), &folder, &repair);
                        if !outcome.passed {
                            outcomes.push(outcome);
                            continue;
                        }
                    }
                }
                outcomes.push(run(&shown, pack.label(), &folder, &check));
            }
        }
        for pack in packs {
            for drift in pack.drift(root, &folder, fixing, &mut notes) {
                outcomes.push(Outcome {
                    unit: shown.clone(),
                    pack: pack.label(),
                    check: drift.check,
                    passed: drift.problems.is_empty(),
                    seconds: drift.seconds,
                    tail: drift.problems.join("\n"),
                });
            }
        }
    }
    (outcomes, notes)
}

struct Check {
    name: String,
    run: Vec<String>,
    fix: Option<Vec<String>>,
}

// The pack's configuration files, written outside the judged repository so a run
// changes nothing in it, at a place {pack} in a command resolves to.
fn materialized(pack: &dyn Pack) -> Option<std::path::PathBuf> {
    let configs = pack.configs();
    if configs.is_empty() {
        return None;
    }
    let folder = std::env::temp_dir()
        .join("arbiter-packs")
        .join(pack.label());
    std::fs::create_dir_all(&folder).ok()?;
    for (name, content) in configs {
        std::fs::write(folder.join(name), content).ok()?;
    }
    Some(folder)
}

fn filled(run: &[String], home: Option<&Path>) -> Vec<String> {
    let Some(home) = home else {
        return run.to_vec();
    };
    let shown = home.to_string_lossy();
    run.iter()
        .map(|word| word.replace("{pack}", &shown))
        .collect()
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
            let run = words(entry.get("run")?)?;
            let fix = entry.get("fix").and_then(words);
            (!run.is_empty()).then_some(Check { name, run, fix })
        })
        .collect()
}

fn words(list: &toml::Value) -> Option<Vec<String>> {
    Some(
        list.as_array()?
            .iter()
            .filter_map(|word| word.as_str().map(str::to_string))
            .collect(),
    )
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
