use std::fs;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;

use crate::core::graph::Graph;
use crate::judge::brief;
use crate::judge::brief::Move;
use crate::judge::brief::Trial;
use crate::judge::plans;

// The examiner: performs a case's trial itself - drives the service through
// its door, reads the store raw over its own connection - and writes receipts.
// The repository's tests are never read, run or trusted here.

pub struct Step {
    pub act: String,
    pub saw: String,
    pub pass: bool,
}

pub struct Testimony {
    pub case: String,
    pub needs: Vec<String>,
    pub steps: Vec<Step>,
    pub passed: bool,
}

pub const SEQUENCE: &str = "persistence/1";

pub fn run(found: &Graph) -> Result<Vec<Testimony>, String> {
    let reading = brief::read(found).ok_or("no brief written; a trial starts from a case")?;
    if reading.trials.is_empty() {
        return Err("the brief declares no trial; a case gains one as [case.trial]".to_string());
    }
    let Some(Ok(plans)) = plans::read(&found.root) else {
        return Err("no plans record; a trial runs in the declared world".to_string());
    };
    if Command::new("sqlite3").arg("--version").output().is_err() {
        return Err("sqlite3 is not on this machine; a machine gap is no finding".to_string());
    }
    let mut testimonies = Vec::new();
    for trial in &reading.trials {
        if trial.sequence != "persistence" {
            return Err(format!(
                "the sequence '{}' is not in the law-text; persistence alone is choreographed",
                trial.sequence
            ));
        }
        if trial.store != "sqlite" {
            return Err(format!(
                "the store '{}' has no driver yet; sqlite alone is driven",
                trial.store
            ));
        }
        let needs: Vec<String> = reading
            .needs
            .iter()
            .filter(|need| need.case == trial.case && trial.proves.contains(&need.need))
            .map(|need| need.need.clone())
            .collect();
        if needs.is_empty() {
            return Err(format!(
                "the trial of case '{}' proves nothing the case states; a trial serves stated needs",
                trial.case
            ));
        }
        let steps = perform(found, &plans, trial)?;
        let passed = steps.iter().all(|step| step.pass);
        testimonies.push(Testimony {
            case: trial.case.clone(),
            needs,
            steps,
            passed,
        });
    }
    written(&found.root, &testimonies);
    Ok(testimonies)
}

// One trial, whole: world up, the moves, the raw probes, world down. A failed
// step ends the trial - the world after it no longer answers for the claim.
fn perform(found: &Graph, plans: &plans::Reading, trial: &Trial) -> Result<Vec<Step>, String> {
    let service = plans
        .services
        .iter()
        .find(|service| service.volumes.contains(&trial.volume))
        .ok_or(format!("no service mounts volume {}", trial.volume))?;
    let unit = service
        .unit
        .clone()
        .ok_or(format!("service {} names no unit to build", service.name))?;
    let unit = if unit == "." { String::new() } else { unit };
    let launch = found
        .packs
        .iter()
        .find_map(|pack| pack.launch(&found.root, Path::new(&unit)))
        .ok_or(format!(
            "no pack can stand the unit of service {} up",
            service.name
        ))?;
    let (command, extra) = (launch.command, launch.env);
    let base = std::env::temp_dir().join(format!("arbiter-trial-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base)
        .map_err(|error| format!("the trial folder is unwritable: {error}"))?;
    let port = free_port()?;
    let mut env = extra;
    for (key, value) in &service.env {
        let mut resolved = value.replace("{port}", &port.to_string());
        for volume in &plans.volumes {
            let marker = format!("{{volume:{}}}", volume.name);
            if resolved.contains(&marker) {
                let place = base.join(&volume.name);
                let _ = fs::create_dir_all(&place);
                resolved = resolved.replace(&marker, &place.to_string_lossy());
            }
        }
        env.push((key.clone(), resolved));
    }
    let folder = found.root.join(&unit);
    let world = World {
        trial,
        db: base.join(&trial.volume).join(&trial.path),
        port,
        command,
        folder,
        env,
    };
    let mut steps = Vec::new();
    let mut child = launched(&world.command, &world.folder, &world.env, &base)?;
    let opened = door(port);
    steps.push(Step {
        act: format!(
            "the world: volume {} placed, service {} launched, door awaited",
            trial.volume, service.name
        ),
        saw: if opened {
            format!("the door answered on port {port}")
        } else {
            format!("the door never opened; {}", stderr_tail(&base))
        },
        pass: opened,
    });
    if opened {
        acted(&mut steps, &world, &mut child);
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&base);
    Ok(steps)
}

// One trial's standing world: what the examiner needs to act, probe and
// revive the service.
struct World<'a> {
    trial: &'a Trial,
    db: PathBuf,
    port: u16,
    command: Vec<String>,
    folder: PathBuf,
    env: Vec<(String, String)>,
}

// The sequence: each door move is the feature acting; each probe is the
// examiner's own connection reading what truly happened to the store.
fn acted(steps: &mut Vec<Step>, world: &World, child: &mut Child) {
    let trial = world.trial;
    let db = &world.db;
    let port = world.port;
    let mut check = |act: String, saw: String, pass: bool| -> bool {
        steps.push(Step { act, saw, pass });
        pass
    };
    let probed = |expect: &str| -> (String, bool) {
        match probe(db, &trial.probe) {
            Ok(saw) => (format!("the probe read '{saw}'"), saw == expect),
            Err(error) => (error, false),
        }
    };
    let moved = |movement: &Move| -> Result<(u16, String), String> { http(port, movement) };

    if let Err(error) = moved(&trial.create) {
        check("created through the door".to_string(), error, false);
        return;
    }
    let (saw, pass) = probed(&trial.created);
    if !check(
        format!(
            "created through the door ({} {})",
            trial.create.method, trial.create.route
        ),
        saw,
        pass,
    ) {
        return;
    }

    if let Err(error) = moved(&trial.update) {
        check("updated through the door".to_string(), error, false);
        return;
    }
    let (saw, pass) = probed(&trial.updated);
    if !check(
        format!(
            "updated through the door ({} {})",
            trial.update.method, trial.update.route
        ),
        saw,
        pass,
    ) {
        return;
    }

    let _ = child.kill();
    let _ = child.wait();
    let revived = match launched(
        &world.command,
        &world.folder,
        &world.env,
        Path::new("/dev/null"),
    ) {
        Ok(reborn) => {
            *child = reborn;
            door(port)
        }
        Err(_) => false,
    };
    let (saw, pass) = if revived {
        probed(&trial.updated)
    } else {
        ("the service did not come back".to_string(), false)
    };
    if !check(
        "the process was killed and restarted on the same volume".to_string(),
        format!("{saw} after the restart"),
        pass,
    ) {
        return;
    }

    if let Err(error) = moved(&trial.delete) {
        check("forgotten through the door".to_string(), error, false);
        return;
    }
    let (saw, pass) = match probe(db, &trial.probe) {
        Ok(read) if read.is_empty() => ("the probe read nothing".to_string(), true),
        Ok(read) => (format!("the probe still read '{read}'"), false),
        Err(error) => (error, false),
    };
    if !check(
        format!(
            "forgotten through the door ({} {})",
            trial.delete.method, trial.delete.route
        ),
        saw,
        pass,
    ) {
        return;
    }

    if let Err(error) = execute(db, &trial.seed) {
        check("seeded raw by the examiner".to_string(), error, false);
        return;
    }
    let (saw, pass) = match moved(&trial.read) {
        Ok((status, body)) => (
            format!("the door answered {status} with '{}'", body.trim()),
            status == 200 && body.trim() == trial.answer,
        ),
        Err(error) => (error, false),
    };
    check(
        format!(
            "seeded raw by the examiner; read through the door ({} {})",
            trial.read.method, trial.read.route
        ),
        saw,
        pass,
    );
}

fn launched(
    command: &[String],
    folder: &Path,
    env: &[(String, String)],
    base: &Path,
) -> Result<Child, String> {
    let log = fs::File::create(base.join("service.log")).ok().map_or(
        (Stdio::null(), Stdio::null()),
        |file| {
            let twin = file.try_clone().map_or(Stdio::null(), Stdio::from);
            (Stdio::from(file), twin)
        },
    );
    Command::new(&command[0])
        .args(&command[1..])
        .current_dir(folder)
        .envs(env.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .stdout(log.0)
        .stderr(log.1)
        .spawn()
        .map_err(|error| format!("the service could not start: {error}"))
}

fn stderr_tail(base: &Path) -> String {
    let Ok(log) = fs::read_to_string(base.join("service.log")) else {
        return "the service wrote nothing".to_string();
    };
    let tail: Vec<&str> = log.lines().rev().take(3).collect();
    tail.into_iter().rev().collect::<Vec<&str>>().join(" | ")
}

fn free_port() -> Result<u16, String> {
    let held = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("no port to hold a door: {error}"))?;
    held.local_addr()
        .map(|address| address.port())
        .map_err(|error| format!("no port to hold a door: {error}"))
}

fn door(port: u16) -> bool {
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

// A whole exchange on one connection: the server closes after answering, so
// the read ends at the connection's end.
fn http(port: u16, movement: &Move) -> Result<(u16, String), String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .map_err(|error| format!("the door did not answer: {error}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let request = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        movement.method,
        movement.route,
        movement.body.len(),
        movement.body
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("the door closed mid-request: {error}"))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|error| format!("the door closed mid-answer: {error}"))?;
    let answer = String::from_utf8_lossy(&raw);
    let status = answer
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or(format!("the door spoke no HTTP: {answer}"))?;
    let body = answer
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    Ok((status, body))
}

// The examiner's own eyes: the store's first-party client against the file,
// never the repository's code.
fn probe(db: &Path, sql: &str) -> Result<String, String> {
    let read = Command::new("sqlite3")
        .arg(db)
        .arg(sql)
        .output()
        .map_err(|error| format!("sqlite3 did not run: {error}"))?;
    if !read.status.success() {
        return Err(format!(
            "the probe failed: {}",
            String::from_utf8_lossy(&read.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&read.stdout).trim().to_string())
}

fn execute(db: &Path, sql: &str) -> Result<(), String> {
    probe(db, sql).map(|_| ())
}

// The receipts: who testified, what was performed, what was seen - evidence
// beside the report, in the examiner's voice, never the repository's.
fn written(root: &Path, testimonies: &[Testimony]) {
    let folder = root.join(".arbiter").join("last");
    if fs::create_dir_all(&folder).is_err() {
        return;
    }
    let keep = folder.join(".gitignore");
    if !keep.exists() {
        let _ = fs::write(keep, "*\n");
    }
    let listed: Vec<serde_json::Value> = testimonies
        .iter()
        .map(|testimony| {
            serde_json::json!({
                "case": testimony.case,
                "passed": testimony.passed,
                "needs": testimony.needs,
                "steps": testimony.steps.iter().map(|step| {
                    serde_json::json!({ "act": step.act, "saw": step.saw, "pass": step.pass })
                }).collect::<Vec<serde_json::Value>>(),
            })
        })
        .collect();
    let receipts = serde_json::json!({
        "examiner": format!("arbiter {}", crate::verbs::VERSION),
        "driver": "sqlite",
        "sequence": SEQUENCE,
        "trials": listed,
    });
    let _ = fs::write(
        folder.join("receipts.json"),
        serde_json::to_string_pretty(&receipts).unwrap_or_default() + "\n",
    );
}
