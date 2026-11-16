use std::hint::black_box;
use std::io;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::context::task_context::TaskContext;
use crate::declaration::pull::Done;
use crate::declaration::pull::Failed;
use crate::declaration::pull::In;
use crate::declaration::task::Task;
use crate::record::task_failure::TaskFailure;
use crate::test_tasks::values::After;
use crate::test_tasks::values::Answered;
use crate::test_tasks::values::Appetite;
use crate::test_tasks::values::Archived;
use crate::test_tasks::values::Boom;
use crate::test_tasks::values::Bounds;
use crate::test_tasks::values::Chatter;
use crate::test_tasks::values::Checked;
use crate::test_tasks::values::Costly;
use crate::test_tasks::values::Cycles;
use crate::test_tasks::values::Deaf;
use crate::test_tasks::values::Flaked;
use crate::test_tasks::values::Flakiness;
use crate::test_tasks::values::Graph;
use crate::test_tasks::values::Held;
use crate::test_tasks::values::Landed;
use crate::test_tasks::values::Later;
use crate::test_tasks::values::Licenses;
use crate::test_tasks::values::Never;
use crate::test_tasks::values::Number;
use crate::test_tasks::values::Patch;
use crate::test_tasks::values::Printed;
use crate::test_tasks::values::Quick;
use crate::test_tasks::values::Repo;
use crate::test_tasks::values::Reverted;
use crate::test_tasks::values::Root;
use crate::test_tasks::values::Said;
use crate::test_tasks::values::Slept;
use crate::test_tasks::values::Slow;
use crate::test_tasks::values::Source;
use crate::test_tasks::values::Squared;
use crate::test_tasks::values::Summary;
use crate::test_tasks::values::Told;
use crate::test_tasks::values::Tools;
use crate::test_tasks::values::Total;
use crate::test_tasks::values::Turns;
use crate::test_tasks::values::Undone;
use crate::test_tasks::values::Unsummed;

// Tasks with no purpose but to show the runner at work: a gate's shape, a
// repair tried and checked, a list squared item by item and summed, and one
// task for each way an attempt can end. What
// a task does is decided by what it reads, never by how it was built, since
// the process running it is handed only its name and its inputs.

pub fn scan() -> Task {
    Task::new("scan", |context: &mut TaskContext, root: In<Root>| {
        context.note(format!("scanned {}", root.name));
        Ok::<Repo, TaskFailure>(Repo {
            files: root.name.len(),
            parses: root.parses,
        })
    })
}

pub fn resolve() -> Task {
    Task::new("resolve", |_: &mut TaskContext, repo: In<Repo>| {
        if repo.parses {
            Ok(Graph {
                nodes: repo.files * 10,
            })
        } else {
            Err(TaskFailure::from("the parser choked"))
        }
    })
}

pub fn cycles() -> Task {
    Task::new("cycles", |_: &mut TaskContext, graph: In<Graph>| {
        thread::sleep(Duration::from_millis(200));
        Ok::<Cycles, TaskFailure>(Cycles {
            found: graph.nodes + 1,
        })
    })
}

pub fn boundaries() -> Task {
    Task::new("boundaries", |_: &mut TaskContext, graph: In<Graph>| {
        thread::sleep(Duration::from_millis(200));
        Ok::<Bounds, TaskFailure>(Bounds {
            found: graph.nodes + 2,
        })
    })
}

pub fn tools() -> Task {
    Task::new("tools", |context: &mut TaskContext, graph: In<Graph>| {
        let mut passed = Vec::new();
        for check in ["clippy", "test"] {
            context.substep(check, |inner| {
                inner.note(format!("over {} nodes", graph.nodes));
                Ok::<(), TaskFailure>(())
            })?;
            passed.push(check.to_string());
        }
        Ok::<Tools, TaskFailure>(Tools { passed })
    })
}

pub fn ledger() -> Task {
    Task::new(
        "ledger",
        |_: &mut TaskContext, (cycles, bounds, tools): (In<Cycles>, In<Bounds>, In<Tools>)| {
            Ok::<Summary, TaskFailure>(Summary {
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
    Task::new("licenses", |_: &mut TaskContext, root: In<Root>| {
        Ok::<Licenses, TaskFailure>(Licenses {
            bytes: root.name.len(),
        })
    })
}

pub fn fix() -> Task {
    Task::new("fix", |_: &mut TaskContext, source: In<Source>| {
        Ok::<Patch, TaskFailure>(Patch {
            text: format!("{} fixed", source.name),
            compiles: source.compiles,
        })
    })
}

pub fn check() -> Task {
    Task::new("check", |_: &mut TaskContext, patch: In<Patch>| {
        if patch.compiles {
            Ok(Checked)
        } else {
            Err(TaskFailure::from(format!(
                "{} does not compile",
                patch.text
            )))
        }
    })
}

pub fn commit() -> Task {
    Task::new(
        "commit",
        |_: &mut TaskContext, (_checked, _patch): (In<Checked>, In<Patch>)| {
            Ok::<Landed, TaskFailure>(Landed)
        },
    )
}

pub fn revert() -> Task {
    Task::new(
        "revert",
        |context: &mut TaskContext, (bad, patch): (Failed<Checked>, In<Patch>)| {
            context.note(format!(
                "undid {} as {}: {}",
                patch.text,
                bad.attempt_id.path(),
                bad.reason
            ));
            Ok::<Reverted, TaskFailure>(Reverted {
                text: patch.text.clone(),
            })
        },
    )
}

pub fn archive() -> Task {
    Task::new("archive", |_: &mut TaskContext, _: In<Reverted>| {
        Ok::<Archived, TaskFailure>(Archived)
    })
}

pub fn report() -> Task {
    Task::new(
        "report",
        |_: &mut TaskContext, (landed, reverted): (Done<Landed>, Done<Reverted>)| {
            Ok::<Told, TaskFailure>(Told {
                line: format!("{} {}", landed.outcome, reverted.outcome),
            })
        },
    )
}

// Checks in every ten milliseconds for three seconds.
pub fn slow() -> Task {
    Task::new("slow", |context: &mut TaskContext, _: In<Root>| {
        for _ in 0..300 {
            context.checkpoint()?;
            thread::sleep(Duration::from_millis(10));
        }
        Ok::<Slow, TaskFailure>(Slow)
    })
}

pub fn later() -> Task {
    Task::new("later", |_: &mut TaskContext, _: In<Root>| {
        Ok::<Later, TaskFailure>(Later)
    })
}

pub fn after() -> Task {
    Task::new("after", |_: &mut TaskContext, _: In<Slow>| {
        Ok::<After, TaskFailure>(After)
    })
}

pub fn undo() -> Task {
    Task::new("undo", |_: &mut TaskContext, _: Failed<Slow>| {
        Ok::<Undone, TaskFailure>(Undone)
    })
}

pub fn tell() -> Task {
    Task::new(
        "tell",
        |_: &mut TaskContext, (slow, undone): (Done<Slow>, Done<Undone>)| {
            Ok::<Said, TaskFailure>(Said {
                line: format!("{} {}", slow.outcome, undone.outcome),
            })
        },
    )
}

pub fn boom() -> Task {
    Task::new(
        "boom",
        |_: &mut TaskContext, _: In<Root>| -> Result<Boom, TaskFailure> { panic!("boom") },
    )
}

// Fails until the attempt it is told to pass at, with two retries.
pub fn flaky() -> Task {
    Task::new(
        "flaky",
        |context: &mut TaskContext, flakiness: In<Flakiness>| {
            let attempt = context.attempt_id().attempt;
            if attempt < flakiness.passes_at {
                Err(TaskFailure::from(format!("attempt {attempt} failed")))
            } else {
                Ok(Flaked { attempt })
            }
        },
    )
    .retries(2)
}

pub fn never() -> Task {
    Task::new("never", |_: &mut TaskContext, _: In<Root>| {
        Err::<Never, TaskFailure>("no".into())
    })
    .retries(1)
}

pub fn answer() -> Task {
    Task::new("answer", |_: &mut TaskContext, bad: Failed<Never>| {
        Ok::<Answered, TaskFailure>(Answered {
            line: format!("{}#{}", bad.attempt_id.path(), bad.attempt_id.attempt),
        })
    })
}

// Runs a five second command with a tenth of a second allowed.
pub fn sleeper() -> Task {
    Task::new("sleeper", |context: &mut TaskContext, _: In<Root>| {
        context.command(Command::new("sleep").arg("5"))?;
        Ok::<Slept, TaskFailure>(Slept)
    })
    .within(Duration::from_millis(100))
}

pub fn quick() -> Task {
    Task::new("quick", |_: &mut TaskContext, _: In<Root>| {
        Ok::<Quick, TaskFailure>(Quick)
    })
}

// Checks in every ten milliseconds, with fifty allowed.
pub fn looper() -> Task {
    Task::new("looper", |context: &mut TaskContext, _: In<Root>| {
        let mut taken = 0;
        for turn in 0..300 {
            context.checkpoint()?;
            context.note(format!("turn {turn}"));
            thread::sleep(Duration::from_millis(10));
            taken += 1;
        }
        Ok::<Turns, TaskFailure>(Turns { taken })
    })
    .within(Duration::from_millis(50))
}

// Never checks in, with fifty milliseconds allowed.
pub fn deaf() -> Task {
    Task::new("deaf", |_: &mut TaskContext, _: In<Root>| {
        thread::sleep(Duration::from_secs(10));
        Ok::<Deaf, TaskFailure>(Deaf)
    })
    .within(Duration::from_millis(50))
}

// Holds as many mebibytes as it is told, touching every page.
pub fn hungry() -> Task {
    Task::new("hungry", |_: &mut TaskContext, appetite: In<Appetite>| {
        let mut held = vec![0u8; appetite.mebibytes << 20];
        for page in (0..held.len()).step_by(4096) {
            held[page] = 1;
        }
        black_box(&held);
        Ok::<Held, TaskFailure>(Held { bytes: held.len() })
    })
}

// Prints as many bytes as it is told.
pub fn printer() -> Task {
    Task::new("printer", |_: &mut TaskContext, chatter: In<Chatter>| {
        let chunk = vec![b'x'; 65536];
        let mut left = chatter.bytes;
        let out = io::stdout();
        let mut out = out.lock();
        while left > 0 {
            let n = left.min(chunk.len());
            out.write_all(&chunk[..n])
                .map_err(|error| TaskFailure::from(error.to_string()))?;
            left -= n;
        }
        out.flush()
            .map_err(|error| TaskFailure::from(error.to_string()))?;
        Ok::<Printed, TaskFailure>(Printed {
            bytes: chatter.bytes,
        })
    })
}

// Computed once per distinct root and taken from the store after.
pub fn costly() -> Task {
    Task::new("costly", |_: &mut TaskContext, _: In<Root>| {
        let computed_at_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|since| u64::try_from(since.as_nanos()).ok())
            .unwrap_or(0);
        Ok::<Costly, TaskFailure>(Costly { computed_at_nanos })
    })
    .cached()
}

// The square of one number, refused below zero; cached, so an unchanged item
// is replayed on its own.
pub fn square() -> Task {
    Task::new("square", |context: &mut TaskContext, number: In<Number>| {
        if number.value < 0 {
            return Err(TaskFailure::from(format!(
                "{} has no square here",
                number.value
            )));
        }
        context.note(format!("squared {}", number.value));
        Ok(Squared {
            value: number.value * number.value,
        })
    })
    .cached()
}

// The same square after a pause long enough to tell items run side by side.
pub fn slow_square() -> Task {
    Task::new("slow_square", |_: &mut TaskContext, number: In<Number>| {
        thread::sleep(Duration::from_millis(300));
        Ok::<Squared, TaskFailure>(Squared {
            value: number.value * number.value,
        })
    })
}

pub fn sum() -> Task {
    Task::new("sum", |_: &mut TaskContext, squares: In<Vec<Squared>>| {
        Ok::<Total, TaskFailure>(Total {
            value: squares.iter().map(|square| square.value).sum(),
        })
    })
}

// Why the squares could not all be had.
pub fn unsummed() -> Task {
    Task::new(
        "unsummed",
        |_: &mut TaskContext, failed: Failed<Vec<Squared>>| {
            Ok::<Unsummed, TaskFailure>(Unsummed {
                reason: failed.reason.clone(),
            })
        },
    )
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
        square(),
        slow_square(),
        sum(),
        unsummed(),
    ]
}
