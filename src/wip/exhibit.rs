use std::hint::black_box;
use std::io;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde::Serialize;

use crate::wip::plan::Plan;
use crate::wip::run::runner::run_within;
use crate::wip::store::Store;
use crate::wip::task::Done;
use crate::wip::task::Failed;
use crate::wip::task::In;
use crate::wip::task::Task;
use crate::wip::trace::Failure;
use crate::wip::trace::Scope;

// Tasks with no purpose but to show the runner at work: a gate's shape, a
// repair tried and judged, and one task for each way an attempt can end. What
// a task does is decided by what it reads, never by how it was built, since
// the process running it is handed only its name and its inputs.

#[derive(Serialize, Deserialize)]
pub struct Root {
    pub name: String,
    pub parses: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Repo {
    pub files: usize,
    pub parses: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Graph {
    pub nodes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Cycles {
    pub found: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Bounds {
    pub found: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Tools {
    pub passed: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Summary {
    pub line: String,
}

#[derive(Serialize, Deserialize)]
pub struct Licenses {
    pub bytes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Verdict {
    pub findings: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Source {
    pub name: String,
    pub compiles: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Patch {
    pub text: String,
    pub compiles: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Checked;

#[derive(Serialize, Deserialize)]
pub struct Landed;

#[derive(Serialize, Deserialize)]
pub struct Reverted {
    pub text: String,
}

#[derive(Serialize, Deserialize)]
pub struct Archived;

#[derive(Serialize, Deserialize)]
pub struct Told {
    pub line: String,
}

#[derive(Serialize, Deserialize)]
pub struct Slow;

#[derive(Serialize, Deserialize)]
pub struct Later;

#[derive(Serialize, Deserialize)]
pub struct After;

#[derive(Serialize, Deserialize)]
pub struct Undone;

#[derive(Serialize, Deserialize)]
pub struct Said {
    pub line: String,
}

#[derive(Serialize, Deserialize)]
pub struct Boom;

#[derive(Serialize, Deserialize)]
pub struct Flakiness {
    pub passes_at: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Flaked {
    pub attempt: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Never;

#[derive(Serialize, Deserialize)]
pub struct Answered {
    pub line: String,
}

#[derive(Serialize, Deserialize)]
pub struct Slept;

#[derive(Serialize, Deserialize)]
pub struct Quick;

#[derive(Serialize, Deserialize)]
pub struct Turns {
    pub taken: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Deaf;

#[derive(Serialize, Deserialize)]
pub struct Appetite {
    pub mebibytes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Held {
    pub bytes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Chatter {
    pub bytes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Printed {
    pub bytes: usize,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Costly {
    pub computed_at_nanos: u64,
}

pub fn scan() -> Task {
    Task::new("scan", |scope: &mut Scope, root: In<Root>| {
        scope.note(format!("scanned {}", root.name));
        Ok::<Repo, Failure>(Repo {
            files: root.name.len(),
            parses: root.parses,
        })
    })
}

pub fn resolve() -> Task {
    Task::new("resolve", |_: &mut Scope, repo: In<Repo>| {
        if repo.parses {
            Ok(Graph {
                nodes: repo.files * 10,
            })
        } else {
            Err(Failure::from("the parser choked"))
        }
    })
}

pub fn cycles() -> Task {
    Task::new("cycles", |_: &mut Scope, graph: In<Graph>| {
        thread::sleep(Duration::from_millis(200));
        Ok::<Cycles, Failure>(Cycles {
            found: graph.nodes + 1,
        })
    })
}

pub fn boundaries() -> Task {
    Task::new("boundaries", |_: &mut Scope, graph: In<Graph>| {
        thread::sleep(Duration::from_millis(200));
        Ok::<Bounds, Failure>(Bounds {
            found: graph.nodes + 2,
        })
    })
}

pub fn tools() -> Task {
    Task::new("tools", |scope: &mut Scope, graph: In<Graph>| {
        let mut passed = Vec::new();
        for check in ["clippy", "test"] {
            scope.step(check, |inner| {
                inner.note(format!("over {} nodes", graph.nodes));
                Ok::<(), Failure>(())
            })?;
            passed.push(check.to_string());
        }
        Ok::<Tools, Failure>(Tools { passed })
    })
}

pub fn ledger() -> Task {
    Task::new(
        "ledger",
        |_: &mut Scope, (cycles, bounds, tools): (In<Cycles>, In<Bounds>, In<Tools>)| {
            Ok::<Summary, Failure>(Summary {
                line: format!(
                    "{} {} {}",
                    cycles.found,
                    bounds.found,
                    tools.passed.join("+")
                ),
            })
        },
    )
}

pub fn licenses() -> Task {
    Task::new("licenses", |_: &mut Scope, root: In<Root>| {
        Ok::<Licenses, Failure>(Licenses {
            bytes: root.name.len(),
        })
    })
}

// Runs the two laws as a plan of its own, inside this attempt.
pub fn judge() -> Task {
    Task::new("judge", |scope: &mut Scope, graph: In<Graph>| {
        let laws = Plan::new("laws", &listed(), &["cycles", "boundaries"])
            .map_err(|invalid| Failure::from(format!("{invalid:?}")))?;
        let mut seeds = Store::new();
        seeds
            .put(&Graph { nodes: graph.nodes })
            .map_err(|error| Failure::from(error.to_string()))?;
        let report = run_within(scope, &laws, seeds)
            .map_err(|refused| Failure::from(refused.to_string()))?;
        let cycles = report.get::<Cycles>().map_or(0, |found| found.found);
        let bounds = report.get::<Bounds>().map_or(0, |found| found.found);
        Ok::<Verdict, Failure>(Verdict {
            findings: cycles + bounds,
        })
    })
}

pub fn fix() -> Task {
    Task::new("fix", |_: &mut Scope, source: In<Source>| {
        Ok::<Patch, Failure>(Patch {
            text: format!("{} fixed", source.name),
            compiles: source.compiles,
        })
    })
}

pub fn check() -> Task {
    Task::new("check", |_: &mut Scope, patch: In<Patch>| {
        if patch.compiles {
            Ok(Checked)
        } else {
            Err(Failure::from(format!("{} does not compile", patch.text)))
        }
    })
}

pub fn commit() -> Task {
    Task::new(
        "commit",
        |_: &mut Scope, (_checked, _patch): (In<Checked>, In<Patch>)| Ok::<Landed, Failure>(Landed),
    )
}

pub fn revert() -> Task {
    Task::new(
        "revert",
        |scope: &mut Scope, (bad, patch): (Failed<Checked>, In<Patch>)| {
            scope.note(format!(
                "undid {} as {}: {}",
                patch.text, bad.job.path, bad.reason
            ));
            Ok::<Reverted, Failure>(Reverted {
                text: patch.text.clone(),
            })
        },
    )
}

pub fn archive() -> Task {
    Task::new("archive", |_: &mut Scope, _: In<Reverted>| {
        Ok::<Archived, Failure>(Archived)
    })
}

pub fn report() -> Task {
    Task::new(
        "report",
        |_: &mut Scope, (landed, reverted): (Done<Landed>, Done<Reverted>)| {
            Ok::<Told, Failure>(Told {
                line: format!("{:?} {:?}", landed.outcome, reverted.outcome),
            })
        },
    )
}

// Checks in every ten milliseconds for three seconds.
pub fn slow() -> Task {
    Task::new("slow", |scope: &mut Scope, _: In<Root>| {
        for _ in 0..300 {
            scope.checkpoint()?;
            thread::sleep(Duration::from_millis(10));
        }
        Ok::<Slow, Failure>(Slow)
    })
}

pub fn later() -> Task {
    Task::new("later", |_: &mut Scope, _: In<Root>| {
        Ok::<Later, Failure>(Later)
    })
}

pub fn after() -> Task {
    Task::new("after", |_: &mut Scope, _: In<Slow>| {
        Ok::<After, Failure>(After)
    })
}

pub fn undo() -> Task {
    Task::new("undo", |_: &mut Scope, _: Failed<Slow>| {
        Ok::<Undone, Failure>(Undone)
    })
}

pub fn tell() -> Task {
    Task::new(
        "tell",
        |_: &mut Scope, (slow, undone): (Done<Slow>, Done<Undone>)| {
            Ok::<Said, Failure>(Said {
                line: format!("{:?} {:?}", slow.outcome, undone.outcome),
            })
        },
    )
}

pub fn boom() -> Task {
    Task::new(
        "boom",
        |_: &mut Scope, _: In<Root>| -> Result<Boom, Failure> { panic!("boom") },
    )
}

// Fails until the attempt it is told to pass at, with two retries.
pub fn flaky() -> Task {
    Task::new("flaky", |scope: &mut Scope, flakiness: In<Flakiness>| {
        let attempt = scope.job().attempt;
        if attempt < flakiness.passes_at {
            Err(Failure::from(format!("attempt {attempt} failed")))
        } else {
            Ok(Flaked { attempt })
        }
    })
    .retries(2)
}

pub fn never() -> Task {
    Task::new("never", |_: &mut Scope, _: In<Root>| {
        Err::<Never, Failure>("no".into())
    })
    .retries(1)
}

pub fn answer() -> Task {
    Task::new("answer", |_: &mut Scope, bad: Failed<Never>| {
        Ok::<Answered, Failure>(Answered {
            line: format!("{}#{}", bad.job.path, bad.job.attempt),
        })
    })
}

// Runs a five second command with a tenth of a second allowed.
pub fn sleeper() -> Task {
    Task::new("sleeper", |scope: &mut Scope, _: In<Root>| {
        scope.command(Command::new("sleep").arg("5"))?;
        Ok::<Slept, Failure>(Slept)
    })
    .within(Duration::from_millis(100))
}

pub fn quick() -> Task {
    Task::new("quick", |_: &mut Scope, _: In<Root>| {
        Ok::<Quick, Failure>(Quick)
    })
}

// Checks in every ten milliseconds, with fifty allowed.
pub fn looper() -> Task {
    Task::new("looper", |scope: &mut Scope, _: In<Root>| {
        let mut taken = 0;
        for turn in 0..300 {
            scope.checkpoint()?;
            scope.note(format!("turn {turn}"));
            thread::sleep(Duration::from_millis(10));
            taken += 1;
        }
        Ok::<Turns, Failure>(Turns { taken })
    })
    .within(Duration::from_millis(50))
}

// Never checks in, with fifty milliseconds allowed.
pub fn deaf() -> Task {
    Task::new("deaf", |_: &mut Scope, _: In<Root>| {
        thread::sleep(Duration::from_secs(10));
        Ok::<Deaf, Failure>(Deaf)
    })
    .within(Duration::from_millis(50))
}

// Holds as many mebibytes as it is told, touching every page.
pub fn hungry() -> Task {
    Task::new("hungry", |_: &mut Scope, appetite: In<Appetite>| {
        let mut held = vec![0u8; appetite.mebibytes << 20];
        for page in (0..held.len()).step_by(4096) {
            held[page] = 1;
        }
        black_box(&held);
        Ok::<Held, Failure>(Held { bytes: held.len() })
    })
}

// Prints as many bytes as it is told.
pub fn printer() -> Task {
    Task::new("printer", |_: &mut Scope, chatter: In<Chatter>| {
        let chunk = vec![b'x'; 65536];
        let mut left = chatter.bytes;
        let out = io::stdout();
        let mut out = out.lock();
        while left > 0 {
            let n = left.min(chunk.len());
            out.write_all(&chunk[..n])
                .map_err(|error| Failure::from(error.to_string()))?;
            left -= n;
        }
        out.flush()
            .map_err(|error| Failure::from(error.to_string()))?;
        Ok::<Printed, Failure>(Printed {
            bytes: chatter.bytes,
        })
    })
}

// Computed once per distinct root and taken from the store after.
pub fn costly() -> Task {
    Task::new("costly", |_: &mut Scope, _: In<Root>| {
        let computed_at_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|since| u64::try_from(since.as_nanos()).ok())
            .unwrap_or(0);
        Ok::<Costly, Failure>(Costly { computed_at_nanos })
    })
    .cached()
}

pub fn listed() -> Vec<Task> {
    vec![
        scan(),
        resolve(),
        cycles(),
        boundaries(),
        tools(),
        ledger(),
        licenses(),
        judge(),
        fix(),
        check(),
        commit(),
        revert(),
        archive(),
        report(),
        slow(),
        later(),
        after(),
        undo(),
        tell(),
        boom(),
        flaky(),
        never(),
        answer(),
        sleeper(),
        quick(),
        looper(),
        deaf(),
        hungry(),
        printer(),
        costly(),
    ]
}
