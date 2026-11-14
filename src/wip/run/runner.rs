use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use crate::wip::job::Cancel;
use crate::wip::job::RunId;
use crate::wip::layout::RUNNING_FILE;
use crate::wip::layout::RUNS_DIR;
use crate::wip::layout::RUN_FILE;
use crate::wip::layout::SEEDS_DIR;
use crate::wip::layout::STORE_DIR;
use crate::wip::plan::Plan;
use crate::wip::resources::Usage;
use crate::wip::run::driver::Driver;
use crate::wip::run::driver::Timing;
use crate::wip::run::flow::Flow;
use crate::wip::run::medium::Medium;
use crate::wip::run::observer::Event;
use crate::wip::run::observer::Observer;
use crate::wip::run::observer::Quiet;
use crate::wip::run::process::Processes;
use crate::wip::run::refusal::Problem;
use crate::wip::run::refusal::Refused;
use crate::wip::run::report::Report;
use crate::wip::run::sweep::sweep;
use crate::wip::store::Store;
use crate::wip::trace::Outcome;
use crate::wip::trace::Scope;
use crate::wip::trace::Site;

// How often the run looks at its attempts between events.
const TICK: Duration = Duration::from_millis(2);

// What runs plans: this many attempts at once, each as a process of the
// worker binary that must begin its body within the startup allowance, stopped
// by a request and then by force after the grace, its output kept on disk up
// to the cap, and this many runs kept in a root.
pub struct Runner {
    slots: usize,
    worker: PathBuf,
    keep: usize,
    startup: Duration,
    grace: Duration,
    cap: u64,
    identity: OnceLock<u64>,
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
            worker: env::current_exe().unwrap_or_else(|_| PathBuf::from("arbiter")),
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

    pub fn worker(mut self, binary: impl Into<PathBuf>) -> Runner {
        self.worker = binary.into();
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

    // The medium every attempt runs in: a process of the worker binary, in a
    // group of its own for a top-level run.
    fn medium(&self, grouped: bool) -> Processes {
        Processes::new(self.worker.clone(), grouped, self.cap, self.grace)
    }

    // Everything that stands between the plan and a run from these seeds,
    // found before any process starts. Nothing here writes.
    pub fn verify(&self, plan: &Plan, seeds: &Store, root: &Path) -> Vec<Problem> {
        let mut problems = Vec::new();
        for need in plan.required() {
            if !seeds.has(need.id) {
                problems.push(Problem::Unseeded { need });
            }
        }
        for task in &plan.tasks {
            if seeds.has(task.produces.id) {
                problems.push(Problem::Shadowed {
                    need: task.produces,
                    by: task.name.clone(),
                });
            }
        }
        problems.extend(self.medium(true).verify());
        if root.exists() && !root.is_dir() {
            problems.push(Problem::Unwritable {
                path: root.to_path_buf(),
                reason: "it is not a folder".to_string(),
            });
        }
        problems
    }

    // Runs the plan from the seeds under the root. A task starts the moment
    // every task it reads has finished and every read fires, up to the slots,
    // so independent tasks run at once; every attempt leaves its record under
    // the run, and the oldest runs beyond those kept are swept at the end.
    pub fn run(&self, plan: &Plan, seeds: Store, root: &Path) -> Result<Report, Refused> {
        self.run_with(plan, seeds, root, &Cancel::new(), &mut Quiet)
    }

    // Runs the plan until it settles or the token is cancelled, telling the
    // observer each event. Cancelling stops what is out and what was about to
    // go out; only what the stopping itself unlocks still runs, so a failure's
    // readers answer it.
    pub fn run_with(
        &self,
        plan: &Plan,
        seeds: Store,
        root: &Path,
        cancel: &Cancel,
        observer: &mut dyn Observer,
    ) -> Result<Report, Refused> {
        let run = RunId::next();
        let place = Place {
            run,
            prefix: String::new(),
            site: Site {
                dir: root.join(RUNS_DIR).join(run.to_string()),
                store: root.join(STORE_DIR),
            },
            grouped: true,
        };
        let report = self.execute(plan, seeds, cancel, place, observer)?;
        sweep(root, self.keep);
        Ok(report)
    }

    // Runs the plan inside a running attempt: same run, ids descending from
    // that attempt's, its folders under the attempt's, its store shared, the
    // attempt's token stopping the inner tasks too, and each record landing
    // under it as well as in the report.
    pub fn run_within(
        &self,
        scope: &mut Scope,
        plan: &Plan,
        seeds: Store,
    ) -> Result<Report, Refused> {
        let place = Place {
            run: scope.job().run,
            prefix: format!("{}/", scope.id()),
            site: scope.site().clone(),
            grouped: false,
        };
        let report = self.execute(plan, seeds, scope.cancel(), place, &mut Quiet)?;
        for step in report.steps() {
            scope.attach(step.clone());
        }
        Ok(report)
    }

    fn execute(
        &self,
        plan: &Plan,
        mut store: Store,
        cancel: &Cancel,
        place: Place,
        observer: &mut dyn Observer,
    ) -> Result<Report, Refused> {
        let Place {
            run,
            prefix,
            site,
            grouped,
        } = place;
        let problems = self.verify(plan, &store, &site.dir);
        if !problems.is_empty() {
            return Err(Refused { problems });
        }
        let begun = Instant::now();
        fs::create_dir_all(&site.dir).map_err(|error| {
            Refused::from(Problem::Unwritable {
                path: site.dir.clone(),
                reason: error.to_string(),
            })
        })?;
        let marker = site.dir.join(RUNNING_FILE);
        if grouped {
            let _ = fs::write(&marker, process::id().to_string());
        }
        let seeds = site.dir.join(SEEDS_DIR);
        store.place(&seeds).map_err(|error| {
            Refused::from(Problem::Unwritable {
                path: seeds,
                reason: error.to_string(),
            })
        })?;
        let dir = site.dir.clone();
        let (flow, decisions) = Flow::new(plan, store, self.identity(), run, &prefix);
        let timing = Timing {
            startup: self.startup,
            grace: self.grace,
            tick: TICK,
        };
        let driver = Driver::new(
            flow,
            decisions,
            self.medium(grouped),
            site,
            self.slots,
            timing,
            observer,
        );
        let (history, store) = driver.drive(cancel).records();
        let mut usage = Usage::default();
        for step in history.iter().flatten() {
            if let Some(account) = &step.account {
                usage.add(&account.usage);
            }
        }
        let failed: Vec<&str> = history
            .iter()
            .filter_map(|attempts| attempts.last())
            .filter(|step| !step.outcome.clean())
            .map(|step| step.job.path.as_str())
            .collect();
        let outcome = if failed.is_empty() {
            Outcome::Ok
        } else {
            Outcome::Failed(format!(
                "{} of {} did not pass: {}",
                failed.len(),
                plan.tasks.len(),
                failed.join(", ")
            ))
        };
        let report = Report {
            run,
            outcome,
            seconds: begun.elapsed().as_secs_f64(),
            usage,
            dir: dir.clone(),
            store,
            names: plan.tasks.iter().map(|task| task.name.clone()).collect(),
            attempts: history,
        };
        let _ = report.write(&dir.join(RUN_FILE));
        if grouped {
            let _ = fs::remove_file(&marker);
        }
        observer.event(Event::Finished { report: &report });
        Ok(report)
    }

    // What the worker binary is, as a number: a product is replayed only for
    // the same code, so a rebuilt binary computes everything again.
    fn identity(&self) -> u64 {
        *self.identity.get_or_init(|| {
            let mut hasher = DefaultHasher::new();
            fs::read(&self.worker).unwrap_or_default().hash(&mut hasher);
            hasher.finish()
        })
    }
}

pub fn run(plan: &Plan, seeds: Store, root: &Path) -> Result<Report, Refused> {
    Runner::new().run(plan, seeds, root)
}

pub fn run_within(scope: &mut Scope, plan: &Plan, seeds: Store) -> Result<Report, Refused> {
    Runner::new().run_within(scope, plan, seeds)
}

// Where a run takes place: its id, the path every task's path starts with,
// its folders, and whether each attempt leads a process group of its own.
struct Place {
    run: RunId,
    prefix: String,
    site: Site,
    grouped: bool,
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    use serde::Deserialize;
    use serde::Serialize;

    use super::Runner;
    use crate::wip::job::RunId;
    use crate::wip::layout::RUNS_DIR;
    use crate::wip::layout::STORE_DIR;
    use crate::wip::plan::Plan;
    use crate::wip::run::refusal::Problem;
    use crate::wip::store::Need;
    use crate::wip::store::Store;
    use crate::wip::task::In;
    use crate::wip::task::Task;
    use crate::wip::trace::Failure;
    use crate::wip::trace::Scope;

    #[derive(Serialize, Deserialize)]
    struct Root(String);

    fn root(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("arbiter-runner-{name}-{}", RunId::next()));
        fs::create_dir_all(dir.join(RUNS_DIR)).expect("a root is made");
        fs::create_dir_all(dir.join(STORE_DIR)).expect("a store is made");
        dir
    }

    #[test]
    fn a_run_is_refused_with_every_problem_named_and_nothing_started() {
        let root = root("refused");
        let known = [Task::new("scan", |_: &mut Scope, root: In<Root>| {
            Ok::<usize, Failure>(root.0.len())
        })];
        let plan = Plan::new("gate", &known, &["scan"]).expect("valid");
        // The type scan produces is seeded, and the type it reads is not.
        let mut seeds = Store::new();
        seeds.put(&4usize).expect("encodes");
        let missing = root.join("missing");
        let flat = root.join("flat");
        fs::write(&flat, "").expect("a file is written");
        let runner = Runner::new().worker(missing.clone());
        let problems = runner.verify(&plan, &seeds, &flat);
        assert_eq!(
            problems[..2],
            [
                Problem::Unseeded {
                    need: Need::of::<Root>()
                },
                Problem::Shadowed {
                    need: Need::of::<usize>(),
                    by: "scan".to_string()
                },
            ]
        );
        assert!(matches!(&problems[2], Problem::NoWorker { path, .. } if path == &missing));
        assert!(matches!(&problems[3], Problem::Unwritable { path, .. } if path == &flat));
        assert_eq!(problems.len(), 4);
        let refused = runner.run(&plan, seeds, &root).err().expect("refused");
        assert_eq!(refused.problems, problems[..3]);
        assert_eq!(refused.to_string().lines().count(), 3);
        assert_eq!(
            fs::read_dir(root.join(RUNS_DIR)).expect("readable").count(),
            0,
            "no run began"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
