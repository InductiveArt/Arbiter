use std::any::Any;
use std::cmp::Reverse;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::env;
use std::fmt;
use std::fs;
use std::fs::File;
use std::hash::Hash;
use std::hash::Hasher;
use std::io;
use std::io::Read;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::wip::job::Cancel;
use crate::wip::job::JobId;
use crate::wip::job::RunId;
use crate::wip::plan::Plan;
use crate::wip::resources::alive;
use crate::wip::resources::exited;
use crate::wip::resources::kill;
use crate::wip::resources::own_group;
use crate::wip::resources::reap;
use crate::wip::resources::terminate;
use crate::wip::resources::Account;
use crate::wip::resources::Reaped;
use crate::wip::resources::Usage;
use crate::wip::store::Completion;
use crate::wip::store::Need;
use crate::wip::store::Part;
use crate::wip::store::Product;
use crate::wip::store::Store;
use crate::wip::task::Task;
use crate::wip::trace::terminated;
use crate::wip::trace::Outcome;
use crate::wip::trace::Scope;
use crate::wip::trace::Site;
use crate::wip::trace::Step;
use crate::wip::worker::Input;
use crate::wip::worker::Order;
use crate::wip::worker::ORDER_FILE;
use crate::wip::worker::PRODUCT_FILE;
use crate::wip::worker::RECORD_FILE;
use crate::wip::worker::STARTED_FILE;

// How a root folder is laid out: one folder per run, and the store of
// products kept across runs, each under its key.
pub const RUNS_DIR: &str = "runs";
pub const STORE_DIR: &str = "store";
// Inside a run: the seeds as files, the pid of the run while it goes, and the
// whole report once it ended. Inside an attempt: the key its product is
// stored under, and what the process printed.
pub const SEEDS_DIR: &str = "seeds";
pub const RUNNING_FILE: &str = "running";
pub const RUN_FILE: &str = "run.json";
pub const KEY_FILE: &str = "product.key";
pub const STDOUT_FILE: &str = "stdout";
pub const STDERR_FILE: &str = "stderr";

// How often the run looks at its processes between events.
const TICK: Duration = Duration::from_millis(2);

// What stands between a plan and a run, found before any process starts.
#[derive(Debug, PartialEq)]
pub enum Problem {
    // A type some task reads that no task produces and the seeds do not hold.
    Unseeded { need: Need },
    // A type both seeded and produced: the product would replace the seed.
    Shadowed { need: Need, by: String },
    // The worker binary is missing, not a file, or not executable.
    NoWorker { path: PathBuf, reason: String },
    // The root is not a folder, or the run's folder or seeds could not be written.
    Unwritable { path: PathBuf, reason: String },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Unseeded { need } => {
                write!(f, "{} is read but neither produced nor seeded", need.name)
            }
            Problem::Shadowed { need, by } => {
                write!(f, "{} is seeded and produced by {by}", need.name)
            }
            Problem::NoWorker { path, reason } => {
                write!(f, "the worker {} cannot run: {reason}", path.display())
            }
            Problem::Unwritable { path, reason } => {
                write!(f, "{} cannot be written: {reason}", path.display())
            }
        }
    }
}

// A run refused whole before any task starts, with every problem found.
#[derive(Debug, PartialEq)]
pub struct Refused {
    pub problems: Vec<Problem>,
}

impl From<Problem> for Refused {
    fn from(problem: Problem) -> Refused {
        Refused {
            problems: vec![problem],
        }
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for problem in &self.problems {
            writeln!(f, "{problem}")?;
        }
        Ok(())
    }
}

// How a run ended, the kernel's account of every attempt of its own tasks
// summed, where it lives on disk, what it left in the store, and every
// attempt's record, task by task in plan order.
pub struct Report {
    pub run: RunId,
    pub outcome: Outcome,
    pub seconds: f64,
    pub usage: Usage,
    pub dir: PathBuf,
    pub store: Store,
    names: Vec<String>,
    pub attempts: Vec<Vec<Step>>,
}

impl Report {
    pub fn get<T: DeserializeOwned + Any>(&self) -> Option<T> {
        self.store.get::<T>()
    }

    pub fn attempts_of(&self, name: &str) -> &[Step] {
        self.names
            .iter()
            .position(|known| known == name)
            .map_or(&[], |at| &self.attempts[at])
    }

    // The last attempt's record: how the task ended.
    pub fn step(&self, name: &str) -> Option<&Step> {
        self.attempts_of(name).last()
    }

    // Every task's last record, in plan order.
    pub fn steps(&self) -> Vec<&Step> {
        self.attempts
            .iter()
            .filter_map(|made| made.last())
            .collect()
    }

    fn write(&self, file: &Path) -> io::Result<()> {
        let written = Written {
            run: self.run.to_string(),
            outcome: &self.outcome,
            seconds: self.seconds,
            usage: &self.usage,
            tasks: self
                .names
                .iter()
                .zip(&self.attempts)
                .map(|(name, attempts)| WrittenTask { name, attempts })
                .collect(),
        };
        let bytes = serde_json::to_vec_pretty(&written).map_err(io::Error::other)?;
        fs::write(file, bytes)
    }
}

#[derive(Serialize)]
struct Written<'a> {
    run: String,
    outcome: &'a Outcome,
    seconds: f64,
    usage: &'a Usage,
    tasks: Vec<WrittenTask<'a>>,
}

#[derive(Serialize)]
struct WrittenTask<'a> {
    name: &'a str,
    attempts: &'a [Step],
}

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
        let reason = match fs::metadata(&self.worker) {
            Ok(meta) if !meta.is_file() => Some("it is not a file".to_string()),
            Ok(meta) if meta.permissions().mode() & 0o111 == 0 => {
                Some("it is not executable".to_string())
            }
            Ok(_) => None,
            Err(error) => Some(error.to_string()),
        };
        if let Some(reason) = reason {
            problems.push(Problem::NoWorker {
                path: self.worker.clone(),
                reason,
            });
        }
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
        self.run_with(plan, seeds, root, &Cancel::new())
    }

    // Runs the plan until it settles or the token is cancelled. Cancelling
    // stops what is out and what was about to go out; only what the stopping
    // itself unlocks still runs, so a failure's readers answer it.
    pub fn run_with(
        &self,
        plan: &Plan,
        seeds: Store,
        root: &Path,
        cancel: &Cancel,
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
        let report = self.execute(plan, seeds, cancel, place)?;
        self.sweep(root);
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
        let report = self.execute(plan, seeds, scope.cancel(), place)?;
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
    ) -> Result<Report, Refused> {
        let Place {
            run, site, grouped, ..
        } = &place;
        let (run, grouped) = (*run, *grouped);
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
        let mut schedule = Schedule::new(self, plan, store, place);
        schedule.start();
        schedule.drive(cancel);
        let Schedule { history, store, .. } = schedule;
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

    // Keeps the newest runs and every run still going, deletes the rest, and
    // drops from the store every product no kept run refers to. Only folders
    // named as a run id are touched.
    fn sweep(&self, root: &Path) {
        let Ok(entries) = fs::read_dir(root.join(RUNS_DIR)) else {
            return;
        };
        let mut runs: Vec<(RunId, PathBuf)> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                Some((RunId::parse(&name)?, entry.path()))
            })
            .collect();
        runs.sort_by_key(|run| Reverse(run.0));
        let (kept, beyond) = runs.split_at(self.keep.min(runs.len()));
        let mut referenced: HashSet<String> = HashSet::new();
        for (_, dir) in kept {
            keys_under(dir, &mut referenced);
        }
        for (_, dir) in beyond {
            if running(dir) {
                keys_under(dir, &mut referenced);
            } else {
                let _ = fs::remove_dir_all(dir);
            }
        }
        if let Ok(entries) = fs::read_dir(root.join(STORE_DIR)) {
            for entry in entries.flatten() {
                let Ok(name) = entry.file_name().into_string() else {
                    continue;
                };
                if !referenced.contains(&name) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
}

fn running(dir: &Path) -> bool {
    fs::read_to_string(dir.join(RUNNING_FILE))
        .ok()
        .and_then(|pid| pid.trim().parse::<u32>().ok())
        .is_some_and(alive)
}

fn keys_under(dir: &Path, into: &mut HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            keys_under(&path, into);
        } else if entry.file_name() == KEY_FILE {
            if let Ok(key) = fs::read_to_string(&path) {
                into.insert(key.trim().to_string());
            }
        }
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

// An attempt out as a process: which task and slot, its folder, the process,
// when it was queued and spawned, when its body could begin, how long it may
// take from then, whether it has been told to stop and when, and the pumps
// carrying its output to disk.
struct Live {
    id: JobId,
    task: usize,
    slot: usize,
    dir: PathBuf,
    child: Child,
    queued: Instant,
    spawned: Instant,
    started: Option<Instant>,
    began: SystemTime,
    timeout: Option<Duration>,
    stopping: Option<(Outcome, Instant)>,
    killed: bool,
    stdout: Pump,
    stderr: Pump,
}

// Carries one pipe to a file as the process writes it: everything is read so
// the process never stalls, up to the cap is kept, and both counts come back
// once the pipe closes.
struct Pump {
    counts: Receiver<(u64, u64)>,
}

impl Pump {
    fn finish(self, within: Duration) -> Option<(u64, u64)> {
        self.counts.recv_timeout(within).ok()
    }
}

fn pump(pipe: Option<impl Read + Send + 'static>, into: PathBuf, cap: u64) -> Pump {
    let (send, counts) = mpsc::channel();
    thread::spawn(move || {
        let mut seen: u64 = 0;
        let mut kept: u64 = 0;
        if let Some(mut pipe) = pipe {
            let mut file = File::create(&into).ok();
            let mut chunk = [0u8; 65536];
            loop {
                let n = match pipe.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                seen += n as u64;
                let room = usize::try_from(cap.saturating_sub(kept))
                    .unwrap_or(usize::MAX)
                    .min(n);
                if room > 0 {
                    if let Some(file) = file.as_mut() {
                        if file.write_all(&chunk[..room]).is_ok() {
                            kept += room as u64;
                        }
                    }
                }
            }
        }
        let _ = send.send((seen, kept));
    });
    Pump { counts }
}

fn sized(file: &Path) -> (u64, u64) {
    let bytes = fs::metadata(file).map(|meta| meta.len()).unwrap_or(0);
    (bytes, bytes)
}

// The last thing a process said, leaving aside a runtime's advice on how to
// hear more.
fn last_line(file: &Path) -> String {
    fs::read_to_string(file)
        .ok()
        .and_then(|text| {
            text.lines()
                .rev()
                .find(|line| !line.trim().is_empty() && !line.starts_with("note:"))
                .map(str::to_string)
        })
        .unwrap_or_default()
}

// One run's bookkeeping: what waits on what, what is queued for a slot, what
// is out as a process, what each attempt left, and the loop that settles
// every task from what comes back.
struct Schedule<'a> {
    runner: &'a Runner,
    run: RunId,
    prefix: String,
    plan: &'a Plan,
    store: Store,
    site: Site,
    grouped: bool,
    waits: Vec<usize>,
    tried: Vec<u32>,
    keys: Vec<Option<String>>,
    history: Vec<Vec<Step>>,
    settled: Vec<bool>,
    left: usize,
    queue: VecDeque<(usize, Instant)>,
    pending: VecDeque<(usize, Outcome)>,
    out: Vec<Live>,
    free: BTreeSet<usize>,
    cancelled: bool,
}

impl<'a> Schedule<'a> {
    fn new(runner: &'a Runner, plan: &'a Plan, store: Store, place: Place) -> Self {
        let count = plan.tasks.len();
        Schedule {
            runner,
            run: place.run,
            prefix: place.prefix,
            plan,
            store,
            site: place.site,
            grouped: place.grouped,
            waits: plan.waits(),
            tried: vec![0; count],
            keys: vec![None; count],
            history: (0..count).map(|_| Vec::new()).collect(),
            settled: vec![false; count],
            left: count,
            queue: VecDeque::new(),
            pending: VecDeque::new(),
            out: Vec::new(),
            free: (0..runner.slots).collect(),
            cancelled: false,
        }
    }

    fn path(&self, at: usize) -> String {
        format!(
            "{}{}/{}",
            self.prefix, self.plan.name, self.plan.tasks[at].name
        )
    }

    fn attempt_dir(&self, at: usize, attempt: u32) -> PathBuf {
        self.site
            .dir
            .join(&self.plan.name)
            .join(format!("{}#{attempt}", self.plan.tasks[at].name))
    }

    fn start(&mut self) {
        let ready: Vec<usize> = (0..self.waits.len())
            .filter(|&at| self.waits[at] == 0)
            .collect();
        for at in ready {
            self.ready(at);
        }
    }

    // Every producer has finished: the task's outcome is already known and it
    // finishes without running, its product is in the store and is replayed,
    // or it waits for a slot.
    fn ready(&mut self, at: usize) {
        let task = &self.plan.tasks[at];
        if let Some(outcome) = self.store.settled(&task.reads) {
            self.pending.push_back((at, outcome));
            return;
        }
        if task.cached {
            let key = self.key(task, &self.store.pull(&task.reads));
            if let Ok(product) = Product::from_file(self.site.store.join(&key)) {
                self.replay(at, &key, product);
                return;
            }
            self.keys[at] = Some(key);
        }
        self.queue.push_back((at, Instant::now()));
    }

    // What a product is stored under: the worker binary, the task's name and
    // every input, so the same code on the same inputs finds it again.
    fn key(&self, task: &Task, parts: &[Part]) -> String {
        let mut hasher = DefaultHasher::new();
        self.runner.identity().hash(&mut hasher);
        task.name.hash(&mut hasher);
        for (read, part) in task.reads.iter().zip(parts) {
            read.when.hash(&mut hasher);
            part.value
                .as_ref()
                .map(|product| &*product.bytes)
                .hash(&mut hasher);
            part.completion
                .as_ref()
                .map(|completion| format!("{:?}", completion.outcome))
                .hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }

    // The product is taken from the store: a record says so, and no process runs.
    fn replay(&mut self, at: usize, key: &str, product: Product) {
        let name = &self.plan.tasks[at].name;
        let mut step = Step::unrun(JobId::new(self.run, self.path(at), 0), name, Outcome::Ok);
        step.began = Some(SystemTime::now());
        step.replayed = true;
        let dir = self.attempt_dir(at, 0);
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(dir.join(KEY_FILE), key);
        if let Ok(bytes) = serde_json::to_vec_pretty(&step) {
            let _ = fs::write(dir.join(RECORD_FILE), bytes);
        }
        self.history[at].push(step);
        self.settle(at, Outcome::Ok, Some(product));
    }

    // Starts an attempt as a process: the order is written into its folder,
    // the worker is told the folder, and its output is pumped to disk.
    fn spawn(&mut self, at: usize, queued: Instant, slot: usize) {
        self.tried[at] += 1;
        let attempt = self.tried[at];
        let plan = self.plan;
        let task = &plan.tasks[at];
        let id = JobId::new(self.run, self.path(at), attempt);
        let dir = self.attempt_dir(at, attempt);
        let parts = self.store.pull(&task.reads);
        let order = Order {
            id: id.clone(),
            task: task.name.clone(),
            store: self.site.store.clone(),
            inputs: task
                .reads
                .iter()
                .zip(parts)
                .map(|(read, part)| Input {
                    need: read.need.name.to_string(),
                    when: read.when,
                    value: part.value.and_then(|product| product.file),
                    completion: part.completion,
                })
                .collect(),
        };
        let mut command = Command::new(&self.runner.worker);
        command
            .arg("worker")
            .arg(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if self.grouped {
            own_group(&mut command);
        }
        let placed = fs::create_dir_all(&dir)
            .and_then(|()| serde_json::to_vec_pretty(&order).map_err(io::Error::other))
            .and_then(|bytes| fs::write(dir.join(ORDER_FILE), bytes));
        let began = SystemTime::now();
        let spawned = Instant::now();
        match placed.and_then(|()| command.spawn()) {
            Ok(mut child) => {
                let stdout = pump(child.stdout.take(), dir.join(STDOUT_FILE), self.runner.cap);
                let stderr = pump(child.stderr.take(), dir.join(STDERR_FILE), self.runner.cap);
                self.out.push(Live {
                    id,
                    task: at,
                    slot,
                    dir,
                    child,
                    queued,
                    spawned,
                    started: None,
                    began,
                    timeout: task.timeout,
                    stopping: None,
                    killed: false,
                    stdout,
                    stderr,
                });
            }
            Err(error) => {
                self.free.insert(slot);
                let mut step = Step::unrun(
                    id,
                    &task.name,
                    Outcome::Failed(format!("the worker could not start: {error}")),
                );
                step.began = Some(began);
                step.slot = Some(slot);
                step.wait = Some(spawned.duration_since(queued));
                self.ended(at, step, None);
            }
        }
    }

    fn unrun(&self, at: usize, outcome: Outcome) -> Step {
        Step::unrun(
            JobId::new(self.run, self.path(at), 0),
            &self.plan.tasks[at].name,
            outcome,
        )
    }

    fn drive(&mut self, cancel: &Cancel) {
        loop {
            while let Some((at, outcome)) = self.pending.pop_front() {
                let step = self.unrun(at, outcome.clone());
                self.history[at].push(step);
                self.settle(at, outcome, None);
            }
            if self.left == 0 {
                break;
            }
            if !self.cancelled && (cancel.cancelled() || terminated()) {
                self.cancel_all();
                continue;
            }
            while let (Some(&slot), Some(_)) = (self.free.first(), self.queue.front()) {
                let (at, queued) = self.queue.pop_front().expect("a queued task was seen");
                self.free.remove(&slot);
                self.spawn(at, queued, slot);
            }
            let now = Instant::now();
            self.notice_started(now);
            self.stop_due(now);
            if self.reap_ended() || self.left == 0 || !self.pending.is_empty() {
                continue;
            }
            assert!(!self.out.is_empty(), "a valid plan settles every task");
            thread::sleep(TICK);
        }
    }

    // Notes each process that marked itself ready for its body, so its time
    // allowed counts from there and no request to stop reaches it before it
    // can hear one.
    fn notice_started(&mut self, now: Instant) {
        for live in &mut self.out {
            if live.started.is_none() && live.dir.join(STARTED_FILE).exists() {
                live.started = Some(now);
            }
        }
    }

    // Asks every attempt past its time to stop, and forces every attempt that
    // did not stop within the grace. The time allowed counts from when the
    // body could begin, so no request reaches a body before it can hear one;
    // a process that has not begun within the startup allowance is stopped
    // all the same.
    fn stop_due(&mut self, now: Instant) {
        let grouped = self.grouped;
        let grace = self.runner.grace;
        let startup = self.runner.startup;
        for live in &mut self.out {
            match live.stopping {
                None => {
                    let due = match live.started {
                        Some(started) => live.timeout.map(|allowed| started + allowed),
                        None => Some(live.spawned + startup),
                    };
                    if due.is_some_and(|due| due <= now) {
                        let _ = terminate(&live.child, grouped);
                        live.stopping = Some((Outcome::TimedOut, now));
                    }
                }
                Some((_, since)) => {
                    if !live.killed && since + grace <= now {
                        let _ = kill(&live.child, grouped);
                        live.killed = true;
                    }
                }
            }
        }
    }

    // Reaps every process that ended. A group leader that ended is seen
    // before it is reaped, while its pid is still held, so what it left
    // behind in its group is ended with it.
    fn reap_ended(&mut self) -> bool {
        let grouped = self.grouped;
        let mut any = false;
        let mut i = 0;
        while i < self.out.len() {
            let live = &mut self.out[i];
            let result = match exited(&live.child) {
                Ok(true) => {
                    if grouped {
                        let _ = kill(&live.child, true);
                    }
                    reap(&mut live.child, true)
                }
                Ok(false) => reap(&mut live.child, false),
                Err(error) => Err(error),
            };
            match result {
                Ok(None) => i += 1,
                Ok(Some(reaped)) => {
                    let live = self.out.swap_remove(i);
                    self.finish(live, Ok(reaped));
                    any = true;
                }
                Err(error) => {
                    let live = self.out.swap_remove(i);
                    self.finish(live, Err(error));
                    any = true;
                }
            }
        }
        any
    }

    // An attempt's process ended: its record is read back if it left one,
    // the run's authority over its outcome applied, and what only the run
    // knows added: the slot, the wait, and the kernel's account.
    fn finish(&mut self, live: Live, reaped: io::Result<Reaped>) {
        let ended = Instant::now();
        let grace = self.runner.grace;
        let Live {
            id,
            task: at,
            slot,
            dir,
            queued,
            spawned,
            started,
            began,
            stopping,
            stdout,
            stderr,
            ..
        } = live;
        self.free.insert(slot);
        let (stdout_bytes, stdout_kept) = stdout
            .finish(grace)
            .unwrap_or_else(|| sized(&dir.join(STDOUT_FILE)));
        let (stderr_bytes, stderr_kept) = stderr
            .finish(grace)
            .unwrap_or_else(|| sized(&dir.join(STDERR_FILE)));
        let name = self.plan.tasks[at].name.as_str();
        let record: Option<Step> = fs::read(dir.join(RECORD_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let (exit_code, signal, usage) = match &reaped {
            Ok(reaped) => (reaped.exit_code, reaped.signal, reaped.usage),
            Err(_) => (None, None, Usage::default()),
        };
        let mut outcome = match (&stopping, &record, &reaped) {
            (Some((outcome, _)), _, _) => outcome.clone(),
            (None, Some(step), _) => step.outcome.clone(),
            (None, None, Ok(reaped)) => Outcome::Failed(no_record(reaped, &dir)),
            (None, None, Err(error)) => {
                Outcome::Failed(format!("the worker could not be reaped: {error}"))
            }
        };
        let value = if outcome == Outcome::Ok {
            match self.product(at, &dir) {
                Ok(product) => Some(product),
                Err(reason) => {
                    outcome = Outcome::Failed(reason);
                    None
                }
            }
        } else {
            None
        };
        let mut step = record.unwrap_or_else(|| {
            let mut step = Step::unrun(id.clone(), name, outcome.clone());
            step.resources.wall = ended.duration_since(spawned);
            step
        });
        step.job = id;
        step.outcome = outcome;
        step.began.get_or_insert(began);
        if stopping.is_some() && started.is_none() {
            step.evidence
                .push("stopped before its body began".to_string());
        }
        step.slot = Some(slot);
        step.wait = Some(spawned.duration_since(queued));
        step.account = Some(Account {
            program: self.runner.worker.display().to_string(),
            args: vec!["worker".to_string(), dir.display().to_string()],
            wall: ended.duration_since(spawned),
            startup: started.map(|started| started.duration_since(spawned)),
            exit_code,
            signal,
            killed: stopping.is_some(),
            stdout_bytes,
            stdout_kept,
            stderr_bytes,
            stderr_kept,
            usage,
        });
        self.ended(at, step, value);
    }

    // The product an attempt left, moved into the store under its key when
    // the task is cached. The key is written first, so a sweep never sees a
    // stored product that nothing refers to yet.
    fn product(&self, at: usize, dir: &Path) -> Result<Product, String> {
        let file = dir.join(PRODUCT_FILE);
        let mut product = Product::from_file(file.clone())
            .map_err(|error| format!("the worker passed but left no product: {error}"))?;
        if let Some(key) = &self.keys[at] {
            let kept = self.site.store.join(key);
            let stored = fs::create_dir_all(&self.site.store)
                .and_then(|()| fs::write(dir.join(KEY_FILE), key))
                .and_then(|()| fs::rename(&file, &kept));
            if stored.is_ok() {
                product.file = Some(kept);
            }
        }
        Ok(product)
    }

    // An attempt is over: the task passed, earns another attempt, or is done.
    fn ended(&mut self, at: usize, step: Step, value: Option<Product>) {
        let outcome = step.outcome.clone();
        self.history[at].push(step);
        let again = outcome.failed()
            && outcome != Outcome::Cancelled
            && !self.cancelled
            && self.tried[at] <= self.plan.tasks[at].retries;
        if outcome == Outcome::Ok {
            self.settle(at, outcome, value);
        } else if again {
            self.queue.push_back((at, Instant::now()));
        } else {
            self.settle(at, outcome, None);
        }
    }

    fn settle(&mut self, at: usize, outcome: Outcome, value: Option<Product>) {
        self.settled[at] = true;
        self.left -= 1;
        let job = self.history[at]
            .last()
            .expect("a settled task has a record")
            .job
            .clone();
        self.store.finish(
            self.plan.tasks[at].produces.id,
            Completion { job, outcome },
            value,
        );
        let plan = self.plan;
        for &next in plan.dependents(at) {
            self.waits[next] -= 1;
            if self.waits[next] == 0 {
                self.ready(next);
            }
        }
    }

    // Stops every attempt that is out and ends every one waiting for a slot
    // as cancelled; nothing waiting earns an attempt after this.
    fn cancel_all(&mut self) {
        self.cancelled = true;
        let now = Instant::now();
        let grouped = self.grouped;
        for live in &mut self.out {
            if live.stopping.is_none() {
                let _ = terminate(&live.child, grouped);
                live.stopping = Some((Outcome::Cancelled, now));
            }
        }
        while let Some((at, _)) = self.queue.pop_front() {
            let step = self.unrun(at, Outcome::Cancelled);
            self.history[at].push(step);
            self.settle(at, Outcome::Cancelled, None);
        }
    }
}

fn no_record(reaped: &Reaped, dir: &Path) -> String {
    let how = match (reaped.exit_code, reaped.signal) {
        (Some(code), _) => format!("exited with code {code}"),
        (None, Some(signal)) => format!("was killed by signal {signal}"),
        (None, None) => "ended".to_string(),
    };
    let said = last_line(&dir.join(STDERR_FILE));
    if said.is_empty() {
        format!("the worker {how} and left no record")
    } else {
        format!("the worker {how} and left no record: {said}")
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process;

    use serde::Deserialize;
    use serde::Serialize;

    use super::Problem;
    use super::Runner;
    use super::KEY_FILE;
    use super::RUNNING_FILE;
    use super::RUNS_DIR;
    use super::STORE_DIR;
    use crate::wip::job::RunId;
    use crate::wip::plan::Plan;
    use crate::wip::store::Need;
    use crate::wip::store::Product;
    use crate::wip::store::Store;
    use crate::wip::task::In;
    use crate::wip::task::Task;
    use crate::wip::trace::Failure;
    use crate::wip::trace::Scope;
    use crate::wip::trace::Site;

    #[derive(Serialize, Deserialize)]
    struct Root(String);

    fn root(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("arbiter-runner-{name}-{}", RunId::next()));
        fs::create_dir_all(dir.join(RUNS_DIR)).expect("a root is made");
        fs::create_dir_all(dir.join(STORE_DIR)).expect("a store is made");
        dir
    }

    fn run_dir(root: &Path, run: RunId, keys: &[&str]) -> PathBuf {
        let dir = root.join(RUNS_DIR).join(run.to_string());
        for (n, key) in keys.iter().enumerate() {
            let attempt = dir.join("gate").join(format!("task{n}#1"));
            fs::create_dir_all(&attempt).expect("an attempt folder is made");
            fs::write(attempt.join(KEY_FILE), key).expect("a key is written");
        }
        fs::create_dir_all(&dir).expect("a run folder is made");
        dir
    }

    fn stored(root: &Path, key: &str) {
        fs::write(root.join(STORE_DIR).join(key), b"{}").expect("a product is stored");
    }

    #[test]
    fn a_sweep_keeps_the_newest_runs_and_the_products_they_refer_to_and_drops_the_rest() {
        let root = root("sweep");
        let runs: Vec<RunId> = (0..3).map(|_| RunId::next()).collect();
        let oldest = run_dir(&root, runs[0], &["aaaa"]);
        let middle = run_dir(&root, runs[1], &["bbbb"]);
        let newest = run_dir(&root, runs[2], &["cccc", "bbbb"]);
        for key in ["aaaa", "bbbb", "cccc", "dddd"] {
            stored(&root, key);
        }
        fs::create_dir_all(root.join(RUNS_DIR).join("notes")).expect("a stray folder");
        Runner::new().keep(2).sweep(&root);
        assert!(!oldest.exists(), "the run beyond the kept ones is gone");
        assert!(middle.exists() && newest.exists());
        assert!(
            root.join(RUNS_DIR).join("notes").exists(),
            "only run folders are touched"
        );
        let left: Vec<String> = {
            let mut names: Vec<String> = fs::read_dir(root.join(STORE_DIR))
                .expect("the store is read")
                .flatten()
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect();
            names.sort();
            names
        };
        assert_eq!(left, ["bbbb", "cccc"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_sweep_leaves_a_run_still_going_whatever_its_age() {
        let root = root("going");
        let runs: Vec<RunId> = (0..2).map(|_| RunId::next()).collect();
        let going = run_dir(&root, runs[0], &["aaaa"]);
        fs::write(going.join(RUNNING_FILE), process::id().to_string()).expect("marked");
        let stale = run_dir(&root, runs[1], &[]);
        fs::write(stale.join(RUNNING_FILE), "4000000000").expect("marked");
        stored(&root, "aaaa");
        Runner::new().keep(0).sweep(&root);
        assert!(going.exists(), "a run whose process is alive is kept");
        assert!(
            !stale.exists(),
            "a marker from a process that is gone keeps nothing"
        );
        assert!(root.join(STORE_DIR).join("aaaa").exists());
        let _ = fs::remove_dir_all(&root);
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

    #[test]
    fn a_product_s_key_follows_the_task_and_its_inputs_and_nothing_else() {
        let root = root("keys");
        let known = [Task::new("scan", |_: &mut Scope, root: In<Root>| {
            Ok::<usize, Failure>(root.0.len())
        })];
        let plan = Plan::new("gate", &known, &["scan"]).expect("valid");
        let runner = Runner::new();
        let site = Site {
            dir: root.clone(),
            store: root.join(STORE_DIR),
        };
        let mut seeds = Store::new();
        seeds.put(&Root("repo".to_string())).expect("encodes");
        let place = super::Place {
            run: RunId::next(),
            prefix: String::new(),
            site,
            grouped: true,
        };
        let schedule = super::Schedule::new(&runner, &plan, seeds, place);
        let parts = schedule.store.pull(&plan.tasks[0].reads);
        let once = schedule.key(&plan.tasks[0], &parts);
        let twice = schedule.key(&plan.tasks[0], &parts);
        assert_eq!(once, twice);
        assert_eq!(once.len(), 16);
        let other = vec![crate::wip::store::Part {
            value: Some(Product::encoded(&Root("other".to_string())).expect("encodes")),
            completion: None,
        }];
        assert_ne!(schedule.key(&plan.tasks[0], &other), once);
        let _ = fs::remove_dir_all(&root);
    }
}
