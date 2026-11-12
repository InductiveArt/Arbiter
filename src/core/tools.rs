use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::core::pack::Pack;

pub struct Outcome {
    pub unit: String,
    pub pack: &'static str,
    pub check: String,
    pub passed: bool,
    pub seconds: f64,
    pub output: String,
    // Whether the result is a stored green run under an unchanged key.
    pub warm: bool,
}

pub struct Check {
    pub name: String,
    pub run: Vec<String>,
    pub fix: Option<Vec<String>>,
}

// The pack's configuration files, written outside the judged repository so a run
// changes nothing in it, at a place {pack} in a command resolves to.
pub fn materialized(pack: &dyn Pack) -> Option<std::path::PathBuf> {
    let configs = pack.configs();
    if configs.is_empty() {
        return None;
    }
    let folder = std::env::temp_dir()
        .join("arbiter-packs")
        .join(pack.label());
    // A config a former version of the pack carried would shadow today's, so the
    // folder holds exactly the current set.
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).ok()?;
    for (name, content) in configs {
        std::fs::write(folder.join(name), content).ok()?;
    }
    Some(folder)
}

pub fn filled(run: &[String], home: Option<&Path>) -> Vec<String> {
    let Some(home) = home else {
        return run.to_vec();
    };
    let shown = home.to_string_lossy();
    run.iter()
        .map(|word| word.replace("{pack}", &shown))
        .collect()
}

pub fn checks(adapter: &str) -> Vec<Check> {
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

pub fn executed(unit: &str, pack: &'static str, folder: &Path, check: &Check) -> Outcome {
    let started = Instant::now();
    let finished = Command::new(&check.run[0])
        .args(&check.run[1..])
        .current_dir(folder)
        .output();
    let seconds = started.elapsed().as_secs_f64();
    // The whole output, never a tail: a cut report hides every finding
    // standing behind the last one.
    let (passed, output) = match finished {
        Ok(output) => (
            output.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        ),
        Err(error) => (false, format!("{} did not start: {error}", check.run[0])),
    };
    Outcome {
        unit: unit.to_string(),
        pack,
        check: check.name.clone(),
        passed,
        seconds,
        output: if passed { String::new() } else { output },
        warm: false,
    }
}
