use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use crate::context::stop_token::StopToken;
use crate::contract::RunId;
use crate::contract::RunRecord;
use crate::contract::RunReport;
use crate::contract::Usage;
use crate::declaration::plan::Plan;
use crate::execution::event::Event;
use crate::execution::observer::Observer;
use crate::execution::observer::Quiet;
use crate::execution::run_folder::RUNNING_FILE;
use crate::execution::run_folder::RUNS_DIR;
use crate::execution::run_folder::RUN_FILE;
use crate::execution::run_folder::SEEDS_DIR;
use crate::execution::run_scheduler::RunScheduler;
use crate::execution::site::Site;
use crate::execution::sweep::sweep;
use crate::execution::task_executor::TaskExecutor;
use crate::execution::task_executor::Timing;
use crate::record::refusal::Refusal;
use crate::record::refusal::RefusalReason;
use crate::runtime::process_runtime::ProcessRuntime;
use crate::runtime::task_runtime::TaskRuntime;
use crate::value::identity::Identity;
use crate::value::run_store::RunStore;

// How often the run looks at its attempts between events.
const TICK: Duration = Duration::from_millis(2);

// What runs plans: this many attempts at once, each as a process of the
// task executable that must begin its body within the startup allowance, stopped
// by a request and then by force after the grace, its output kept on disk up
// to the cap, and this many runs kept in a root.
pub struct Runner {
    slots: usize,
    task_executable: PathBuf,
    keep: usize,
    startup: Duration,
    grace: Duration,
    cap: u64,
    identity: OnceLock<Identity>,
}

impl Default for Runner {
    fn default() -> Self {
        Runner::new()
    }
}

impl Runner {
    pub fn new() -> Runner {
        Runner {
            slots: thread::available_parallelism().map_or(4, |cores| cores.get()),
            // The binary running now; when it cannot be told, none, which
            // verifying a run refuses.
            task_executable: env::current_exe().unwrap_or_default(),
            keep: 20,
            startup: Duration::from_secs(10),
            grace: Duration::from_secs(1),
            cap: 16 << 20,
            identity: OnceLock::new(),
        }
    }

    pub fn slots(mut self, slots: usize) -> Runner {
        self.slots = slots.max(1);
        self
    }

    pub fn task_executable(mut self, executable: impl Into<PathBuf>) -> Runner {
        self.task_executable = executable.into();
        self
    }

    pub fn keep(mut self, runs: usize) -> Runner {
        self.keep = runs;
        self
    }

    pub fn startup(mut self, allowance: Duration) -> Runner {
        self.startup = allowance;
        self
    }

    pub fn grace(mut self, grace: Duration) -> Runner {
        self.grace = grace;
        self
    }

    pub fn cap(mut self, bytes: u64) -> Runner {
        self.cap = bytes;
        self
    }

    // The runtime every attempt runs in: a process of the task executable,
    // leading a process group of its own.
    fn runtime(&self) -> ProcessRuntime {
        ProcessRuntime::new(self.task_executable.clone(), self.cap, self.grace)
    }

    // Everything that stands between the plan and a run from these seeds,
    // found before any process starts. Nothing here writes.
    pub fn verify(&self, plan: &Plan, seeds: &RunStore, root: &Path) -> Vec<RefusalReason> {
        let mut reasons = Vec::new();
        for value_type in plan.required() {
            if !seeds.has(value_type.id) {
                reasons.push(RefusalReason::Unseeded { value_type });
            }
        }
        for (at, task) in plan.tasks.iter().enumerate() {
            let produces = plan.produces(at);
            if seeds.has(produces.id) {
                reasons.push(RefusalReason::Shadowed {
                    value_type: produces,
                    by: task.name.clone(),
                });
            }
        }
        reasons.extend(self.runtime().verify());
        if root.exists() && !root.is_dir() {
            reasons.push(RefusalReason::Unwritable {
                path: root.to_path_buf(),
                reason: "it is not a folder".to_string(),
            });
        }
        reasons
    }

    // Runs the plan from the seeds under the root. A task starts the moment
    // every task it reads has finished and every read fires, up to the slots,
    // so independent tasks, and the items of a fanned one, run at once; every
    // attempt leaves its record under the run, and the oldest runs beyond those
    // kept are swept at the end.
    pub fn run(
        &self,
        plan: &Plan,
        seeds: RunStore,
        root: &Path,
    ) -> Result<(RunReport, RunStore), Refusal> {
        self.run_with(plan, seeds, root, &StopToken::new(), &mut Quiet)
    }

    // Runs the plan until it settles or the token is stopped, telling the
    // observer each event. Stopping stops what is out and what was about to
    // go out; only what the stopping itself unlocks still runs, so a failure's
    // readers answer it.
    pub fn run_with(
        &self,
        plan: &Plan,
        mut seeds: RunStore,
        root: &Path,
        stop_token: &StopToken,
        observer: &mut dyn Observer,
    ) -> Result<(RunReport, RunStore), Refusal> {
        let run = RunId::next();
        let site = Site {
            dir: root.join(RUNS_DIR).join(run.to_string()),
            root: root.to_path_buf(),
        };
        let reasons = self.verify(plan, &seeds, &site.dir);
        if !reasons.is_empty() {
            return Err(Refusal { reasons });
        }
        let begun = Instant::now();
        fs::create_dir_all(&site.dir).map_err(|error| {
            Refusal::from(RefusalReason::Unwritable {
                path: site.dir.clone(),
                reason: error.to_string(),
            })
        })?;
        let marker = site.dir.join(RUNNING_FILE);
        let _ = fs::write(&marker, process::id().to_string());
        let held = site.dir.join(SEEDS_DIR);
        seeds.place(&held).map_err(|error| {
            Refusal::from(RefusalReason::Unwritable {
                path: held,
                reason: error.to_string(),
            })
        })?;
        let dir = site.dir.clone();
        let (scheduler, decisions) = RunScheduler::new(plan, seeds, self.identity(), run);
        let timing = Timing {
            startup: self.startup,
            grace: self.grace,
            tick: TICK,
        };
        let executor = TaskExecutor::new(
            scheduler,
            decisions,
            self.runtime(),
            site,
            self.slots,
            timing,
            observer,
        );
        let (invocations, store) = executor.execute(stop_token).records();
        let mut usage = Usage::default();
        for attempt in invocations.iter().flat_map(|invocation| {
            invocation.attempts.iter().chain(
                invocation
                    .instances
                    .iter()
                    .flat_map(|instance| &instance.attempts),
            )
        }) {
            if let Some(process_record) = &attempt.process_record {
                usage.add(process_record.usage());
            }
        }
        let failed: Vec<&str> = invocations
            .iter()
            .filter(|invocation| {
                invocation
                    .attempts
                    .last()
                    .is_some_and(|attempt| !attempt.outcome().clean())
            })
            .map(|invocation| invocation.id().path.as_str())
            .collect();
        let outcome = if failed.is_empty() {
            RunRecord::passed()
        } else {
            RunRecord::failed(format!(
                "{} of {} did not pass: {}",
                failed.len(),
                plan.tasks.len(),
                failed.join(", ")
            ))
        };
        let report = RunReport {
            run: Some(run),
            outcome: Some(outcome),
            seconds: begun.elapsed().as_secs_f64(),
            usage: Some(usage),
            dir: dir.display().to_string(),
            invocations,
        };
        let _ = report.write(&dir.join(RUN_FILE));
        let _ = fs::remove_file(&marker);
        observer.event(Event::Finished { report: &report });
        sweep(root, self.keep);
        Ok((report, store))
    }

    // What the task executable is: an output is replayed only for the same
    // code, so a rebuilt binary computes everything again.
    fn identity(&self) -> Identity {
        self.identity
            .get_or_init(|| Identity::of(&fs::read(&self.task_executable).unwrap_or_default()))
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    use serde::Deserialize;
    use serde::Serialize;

    use super::Runner;
    use crate::context::task_context::TaskContext;
    use crate::contract::RunId;
    use crate::declaration::plan::Plan;
    use crate::declaration::pull::In;
    use crate::declaration::task::Task;
    use crate::execution::run_folder::RUNS_DIR;
    use crate::record::refusal::RefusalReason;
    use crate::record::task_failure::TaskFailure;
    use crate::value::run_store::RunStore;
    use crate::value::value_type::ValueType;

    #[derive(Serialize, Deserialize)]
    struct Root(String);

    fn root(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("task-runner-runner-{name}-{}", RunId::next()));
        fs::create_dir_all(dir.join(RUNS_DIR)).expect("a root is made");
        dir
    }

    #[test]
    fn a_run_is_refused_with_every_reason_named_and_nothing_started() {
        let root = root("refused");
        let known = [Task::new("scan", |_: &mut TaskContext, root: In<Root>| {
            Ok::<usize, TaskFailure>(root.0.len())
        })];
        let plan = Plan::new("gate", &known, &["scan"]).expect("valid");
        // The type scan produces is seeded, and the type it reads is not.
        let mut seeds = RunStore::new();
        seeds.put(&4usize).expect("encodes");
        let missing = root.join("missing");
        let flat = root.join("flat");
        fs::write(&flat, "").expect("a file is written");
        let runner = Runner::new().task_executable(missing.clone());
        let reasons = runner.verify(&plan, &seeds, &flat);
        assert_eq!(
            reasons[..2],
            [
                RefusalReason::Unseeded {
                    value_type: ValueType::of::<Root>()
                },
                RefusalReason::Shadowed {
                    value_type: ValueType::of::<usize>(),
                    by: "scan".to_string()
                },
            ]
        );
        assert!(
            matches!(&reasons[2], RefusalReason::NoTaskExecutable { path, .. } if path == &missing)
        );
        assert!(matches!(&reasons[3], RefusalReason::Unwritable { path, .. } if path == &flat));
        assert_eq!(reasons.len(), 4);
        let refusal = runner.run(&plan, seeds, &root).err().expect("refused");
        assert_eq!(refusal.reasons, reasons[..3]);
        assert_eq!(refusal.to_string().lines().count(), 3);
        assert_eq!(
            fs::read_dir(root.join(RUNS_DIR)).expect("readable").count(),
            0,
            "no run began"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
