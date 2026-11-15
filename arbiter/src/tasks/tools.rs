use std::path::Path;

use crate::core::pack::Finding;
use crate::core::pack::Pack;
use crate::core::pack::Repo;
use crate::core::tools::checks;
use crate::core::tools::executed;
use crate::core::tools::filled;
use crate::core::tools::materialized;
use crate::core::tools::Outcome;

pub fn judged(
    root: &Path,
    repo: &Repo,
    packs: &[Box<dyn Pack>],
    fixing: bool,
    cold: bool,
    warmth: Option<&crate::core::warm::Warmth>,
) -> (Vec<Outcome>, Vec<String>, Vec<Finding>) {
    let mut outcomes = Vec::new();
    let mut notes = Vec::new();
    let mut dialed = Vec::new();
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
        // Dials are findings, and a finding never rests on the store: they are
        // read fresh for every unit, warm or not.
        for pack in packs {
            let Some(manifest) = pack.manifest() else {
                continue;
            };
            let Ok(declared) = std::fs::read_to_string(folder.join(manifest)) else {
                continue;
            };
            if pack.container(&declared) {
                continue;
            }
            if let Some((found, told)) = pack.dials(&unit.path, &folder) {
                dialed.extend(found);
                notes.extend(told.into_iter().map(|note| format!("{shown}: {note}")));
            }
        }
        // A green run stored under an unchanged key answers for the unit's tools;
        // the graph is recomputed regardless, so no finding ever rests on the store.
        if let Some(warmth) = warmth {
            if let Some(stored) = warmth.stored(index, &unit.path) {
                for check in stored {
                    let Some(pack) = packs.iter().find(|pack| pack.label() == check.pack) else {
                        continue;
                    };
                    outcomes.push(Outcome {
                        unit: shown.clone(),
                        pack: pack.label(),
                        check: check.check,
                        passed: true,
                        seconds: check.seconds,
                        output: String::new(),
                        warm: true,
                    });
                }
                continue;
            }
        }
        let first = outcomes.len();
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
            let Some(adapter) = pack.adapter() else {
                notes.push(format!(
                    "{shown} is {}, which declares no tools yet",
                    pack.label()
                ));
                continue;
            };
            let home = materialized(pack.as_ref());
            for check in checks(adapter) {
                let check = crate::core::tools::Check {
                    name: check.name,
                    run: filled(&check.run, home.as_deref()),
                    fix: check.fix.map(|fix| filled(&fix, home.as_deref())),
                };
                if fixing {
                    if let Some(fix) = &check.fix {
                        let repair = crate::core::tools::Check {
                            name: check.name.clone(),
                            run: fix.clone(),
                            fix: None,
                        };
                        let outcome = executed(&shown, pack.label(), &folder, &repair);
                        if !outcome.passed {
                            outcomes.push(outcome);
                            continue;
                        }
                    }
                }
                outcomes.push(executed(&shown, pack.label(), &folder, &check));
            }
        }
        for pack in packs {
            for drift in pack.drift(root, &folder, fixing, cold, &mut notes) {
                outcomes.push(Outcome {
                    unit: shown.clone(),
                    pack: pack.label(),
                    check: drift.check,
                    passed: drift.problems.is_empty(),
                    seconds: drift.seconds,
                    output: drift.problems.join("\n"),
                    warm: false,
                });
            }
        }
        if let Some(warmth) = warmth {
            let mine = &outcomes[first..];
            if mine.is_empty() {
                continue;
            }
            if mine.iter().all(|outcome| outcome.passed) {
                let checks: Vec<crate::core::warm::StoredCheck> = mine
                    .iter()
                    .map(|outcome| crate::core::warm::StoredCheck {
                        check: outcome.check.clone(),
                        pack: outcome.pack.to_string(),
                        seconds: outcome.seconds,
                    })
                    .collect();
                warmth.record(index, &unit.path, &checks);
            } else {
                warmth.invalidate(&unit.path);
            }
        }
    }
    (outcomes, notes, dialed)
}
