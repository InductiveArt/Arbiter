use std::fs;
use std::path::Path;
use std::process::ExitCode;

pub fn run(arguments: &[String]) -> ExitCode {
    if arguments.first().map(String::as_str) == Some("adopt") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let root = std::path::PathBuf::from(&asked);
        if !root.is_dir() {
            eprintln!("adopt: no folder at {asked}");
            return ExitCode::from(2);
        }
        let (told, stale) = adopted(&root);
        for line in told {
            println!("adopt: {line}");
        }
        if stale {
            eprintln!("adopt: a stale binary is a machine gap; update arbiter, then adopt again");
            return ExitCode::from(2);
        }
        return ExitCode::SUCCESS;
    }
    // Dispatched only for this command, so the guard always holds.
    ExitCode::from(2)
}

// The agent layer, installed where agents will look. The judge's own files
// (rules, skill, hook) follow the binary and are refreshed when they drift,
// never backwards: a file stamped by a newer arbiter is left, and the caller
// is told to update. settings.json is the repository's and is written once.
fn adopted(root: &Path) -> (Vec<String>, bool) {
    let mut told = Vec::new();
    let mut stale = false;
    let owned: [(&str, &str); 6] = [
        (
            ".claude/agent-rules.md",
            include_str!("../../share/agent-rules.md"),
        ),
        (
            ".claude/skills/arbiter-quality/SKILL.md",
            include_str!("../../share/skills/arbiter-quality/SKILL.md"),
        ),
        (
            ".claude/skills/arbiter-method/SKILL.md",
            include_str!("../../share/skills/arbiter-method/SKILL.md"),
        ),
        (
            ".claude/skills/arbiter-genesis/SKILL.md",
            include_str!("../../share/skills/arbiter-genesis/SKILL.md"),
        ),
        (
            ".claude/skills/arbiter-arrival/SKILL.md",
            include_str!("../../share/skills/arbiter-arrival/SKILL.md"),
        ),
        (
            ".claude/hooks/arbiter-stop.sh",
            include_str!("../../share/hooks/arbiter-stop.sh"),
        ),
    ];
    for (place, content) in owned {
        let path = root.join(place);
        let expected = stamped(place, content);
        match fs::read_to_string(&path) {
            Ok(present) if present == expected => {
                told.push(format!("kept {place}"));
                continue;
            }
            Ok(present) if ahead(&present) => {
                let (major, minor, patch) = written_by(&present).unwrap_or((0, 0, 0));
                told.push(format!(
                    "left {place}: written by arbiter {major}.{minor}.{patch}, and this \
                     binary is {} — update the binary, not the file",
                    crate::core::VERSION
                ));
                stale = true;
                continue;
            }
            Ok(_) => {
                if fs::write(&path, &expected).is_ok() {
                    told.push(format!("refreshed {place}"));
                }
                continue;
            }
            Err(_) => {}
        }
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::write(&path, &expected).is_ok() {
            told.push(format!("adopted {place}"));
        }
    }
    let settings = root.join(".claude/settings.json");
    if settings.exists() {
        told.push("kept .claude/settings.json".to_string());
    } else {
        if let Some(parent) = settings.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let content = "{\n  \"hooks\": {\n    \"Stop\": [\n      {\n        \"hooks\": [\n          {\n            \"type\": \"command\",\n            \"command\": \"bash \\\"$CLAUDE_PROJECT_DIR/.claude/hooks/arbiter-stop.sh\\\"\"\n          }\n        ]\n      }\n    ]\n  }\n}\n";
        if fs::write(&settings, content).is_ok() {
            told.push("adopted .claude/settings.json".to_string());
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let hook = root.join(".claude/hooks/arbiter-stop.sh");
        let _ = fs::set_permissions(&hook, fs::Permissions::from_mode(0o755));
    }
    let claude = root.join("CLAUDE.md");
    let pointer = "@.claude/agent-rules.md";
    match fs::read_to_string(&claude) {
        Ok(text) if text.contains(pointer) => told.push("kept CLAUDE.md".to_string()),
        Ok(text) => {
            let _ = fs::write(&claude, format!("{}\n{pointer}\n", text.trim_end()));
            told.push("pointed CLAUDE.md at the rules".to_string());
        }
        Err(_) => {
            let _ = fs::write(&claude, format!("{pointer}\n"));
            told.push("wrote CLAUDE.md pointing at the rules".to_string());
        }
    }
    (told, stale)
}

// An adopted file's last line names its writer, in the comment form the
// file's own syntax reads.
pub fn stamped(place: &str, body: &str) -> String {
    let mark = if place.ends_with(".sh") {
        format!("# arbiter {}", crate::core::VERSION)
    } else {
        format!("<!-- arbiter {} -->", crate::core::VERSION)
    };
    format!("{}\n{mark}\n", body.trim_end())
}

pub fn written_by(text: &str) -> Option<(u64, u64, u64)> {
    let line = text.lines().rev().find(|line| !line.trim().is_empty())?;
    let line = line
        .trim()
        .trim_start_matches("<!--")
        .trim_end_matches("-->")
        .trim_start_matches('#')
        .trim();
    triple(line.strip_prefix("arbiter ")?)
}

// Whether a file's stamp names a newer arbiter than this binary: writing
// over it would move the law backwards.
pub fn ahead(text: &str) -> bool {
    match (written_by(text), triple(crate::core::VERSION)) {
        (Some(theirs), Some(mine)) => theirs > mine,
        _ => false,
    }
}

fn triple(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.');
    let numbers = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    if parts.next().is_some() {
        return None;
    }
    Some(numbers)
}
