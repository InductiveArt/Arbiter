use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use crate::context::stop_token::StopToken;
use crate::contract::ProcessRecord;
use crate::contract::Resources;
use crate::contract::RunRecord;
use crate::contract::TaskAttempt;
use crate::contract::TaskAttemptId;
use crate::declaration::read::ReadInput;
use crate::declaration::task::Task;
use crate::execution::decision::Decision;
use crate::execution::event::Event;
use crate::execution::observer::Observer;
use crate::execution::run_folder::CACHE_DIR;
use crate::execution::run_folder::OUTPUTS_DIR;
use crate::execution::run_scheduler::RunScheduler;
use crate::execution::site::Site;
use crate::kernel::clock::nanos;
use crate::kernel::clock::unix_nanos;
use crate::kernel::signal::terminated;
use crate::protocol::attempt_folder::input_file;
use crate::protocol::attempt_folder::ORDER_FILE;
use crate::protocol::attempt_folder::OUTPUT_FILE;
use crate::protocol::attempt_folder::RECORD_FILE;
use crate::protocol::attempt_folder::STDERR_FILE;
use crate::protocol::task_order::TaskOrder;
use crate::protocol::task_order_input::TaskOrderInput;
use crate::runtime::ended::Ended;
use crate::runtime::task_runtime::TaskRuntime;
use crate::value::encoded_value::EncodedValue;
use crate::value::task_cache::TaskCache;
use crate::value::task_cache_key::TaskCacheKey;
use crate::value::task_output_hash::TaskOutputHash;
use crate::value::task_output_store::TaskOutputStore;

// How long an attempt may take to begin its body, how long it is given to
// stop once asked before it is forced, and how often the run looks between
// events.
pub struct Timing {
    pub startup: Duration,
    pub grace: Duration,
    pub tick: Duration,
}

// An attempt wanted and waiting for a slot.
struct Queued {
    invocation: usize,
    attempt: u32,
    key: Option<TaskCacheKey>,
    since: Instant,
}

// An attempt out in the runtime: which task and slot, its folder, the key its
// output is cached under, when it was queued and started, when its body could
// begin, how long it may take from then, and whether it has been told to stop.
struct Live<H> {
    attempt_id: TaskAttemptId,
    invocation: usize,
    slot: usize,
    dir: PathBuf,
    key: Option<TaskCacheKey>,
    handle: H,
    queued: Instant,
    spawned: Instant,
    started: Option<Instant>,
    began: SystemTime,
    timeout: Option<Duration>,
    stopping: Option<(RunRecord, Instant)>,
    killed: bool,
}

// Executes one run: takes the scheduler's decisions, hands attempts to the
// runtime up to the slots, keeps the only clock, reads back what each attempt
// left, keeps its output and names it in the cache, and feeds the scheduler
// what ended, telling the observer each event as it happens.
pub struct TaskExecutor<'a, 'o, R: TaskRuntime> {
    scheduler: RunScheduler<'a>,
    runtime: R,
    site: Site,
    outputs: TaskOutputStore,
    cache: TaskCache,
    timing: Timing,
    free: BTreeSet<usize>,
    queue: VecDeque<Queued>,
    out: Vec<Live<R::TaskHandle>>,
    observer: &'o mut dyn Observer,
    cancelled: bool,
}

impl<'a, 'o, R: TaskRuntime> TaskExecutor<'a, 'o, R> {
    pub fn new(
        scheduler: RunScheduler<'a>,
        decisions: Vec<Decision>,
        runtime: R,
        site: Site,
        slots: usize,
        timing: Timing,
        observer: &'o mut dyn Observer,
    ) -> TaskExecutor<'a, 'o, R> {
        let outputs = TaskOutputStore::at(site.root.join(OUTPUTS_DIR));
        let cache = TaskCache::at(site.root.join(CACHE_DIR));
        let mut executor = TaskExecutor {
            scheduler,
            runtime,
            site,
            outputs,
            cache,
            timing,
            free: (0..slots.max(1)).collect(),
            queue: VecDeque::new(),
            out: Vec::new(),
            observer,
            cancelled: false,
        };
        executor.apply(decisions);
        executor
    }

    // Runs until every task settled, or the token or a signal cancels the run
    // and what the cancel still unlocks has settled too.
    pub fn execute(mut self, stop_token: &StopToken) -> RunScheduler<'a> {
        loop {
            if self.scheduler.settled() {
                break;
            }
            if !self.cancelled && (stop_token.stopped() || terminated()) {
                self.cancelled = true;
                let decisions = self.scheduler.cancel();
                self.apply(decisions);
                continue;
            }
            self.fill();
            let now = Instant::now();
            self.notice_started(now);
            self.stop_due(now);
            if self.poll_ended() || self.scheduler.settled() {
                continue;
            }
            assert!(!self.out.is_empty(), "a valid plan settles every task");
            thread::sleep(self.timing.tick);
        }
        self.scheduler
    }

    // An attempt's folder: under the plan, by its task's name, and for an item
    // of a fanned task by its task's name and the item's key.
    fn attempt_dir(&self, invocation: usize, attempt: u32) -> PathBuf {
        let plan = self.scheduler.plan();
        let task = &plan.tasks[self.scheduler.task_of(invocation)].name;
        let folder = self.site.dir.join(&plan.name);
        match self.scheduler.item_of(invocation) {
            None => folder.join(format!("{task}#{attempt}")),
            Some(key) => folder.join(format!("{task}@{}#{attempt}", folder_safe(key))),
        }
    }

    // Decisions are taken in the order made, those a decision leads to before
    // the ones after it, so a replay's readers queue where the replayed task
    // would have.
    fn apply(&mut self, decisions: Vec<Decision>) {
        let mut pending: VecDeque<Decision> = decisions.into();
        while let Some(decision) = pending.pop_front() {
            let more = match decision {
                Decision::Attempt {
                    invocation,
                    attempt,
                    key,
                } => self.wanted(invocation, attempt, key),
                Decision::Stop { invocation } => self.stop(invocation),
                Decision::Recorded {
                    invocation,
                    attempt,
                } => {
                    if attempt.replayed {
                        recorded(&self.attempt_dir(invocation, 0), &attempt);
                    }
                    self.observer.event(Event::Ended { attempt: &attempt });
                    Vec::new()
                }
                Decision::Settled {
                    invocation,
                    outcome,
                } => {
                    let path = self.scheduler.invocation_id(invocation).path;
                    self.observer.event(Event::Settled {
                        path: &path,
                        outcome: &outcome,
                    });
                    Vec::new()
                }
            };
            for decision in more.into_iter().rev() {
                pending.push_front(decision);
            }
        }
    }

    // An output the cache names under the key, still kept and still hashing to
    // its name, is replayed and no process runs; otherwise the attempt waits
    // for a slot.
    fn wanted(
        &mut self,
        invocation: usize,
        attempt: u32,
        key: Option<TaskCacheKey>,
    ) -> Vec<Decision> {
        if let Some(key) = &key {
            if let Some(hash) = self.cache.get(key) {
                if let Ok(output) = self.outputs.read(&hash) {
                    let _ = fs::create_dir_all(self.attempt_dir(invocation, 0));
                    return self
                        .scheduler
                        .replayed(invocation, key.clone(), hash, output);
                }
            }
        }
        self.queue.push_back(Queued {
            invocation,
            attempt,
            key,
            since: Instant::now(),
        });
        Vec::new()
    }

    // The task's attempt is told to stop if it is out, and ends as cancelled
    // without an attempt if it was still waiting for a slot.
    fn stop(&mut self, invocation: usize) -> Vec<Decision> {
        if let Some(live) = self
            .out
            .iter_mut()
            .find(|live| live.invocation == invocation)
        {
            if live.stopping.is_none() {
                self.runtime.stop(&live.handle);
                live.stopping = Some((RunRecord::cancelled(), Instant::now()));
            }
            return Vec::new();
        }
        if let Some(waiting) = self
            .queue
            .iter()
            .position(|queued| queued.invocation == invocation)
        {
            self.queue.remove(waiting);
            return self.scheduler.withdrawn(invocation);
        }
        Vec::new()
    }

    fn fill(&mut self) {
        while let (Some(&slot), false) = (self.free.first(), self.queue.is_empty()) {
            let queued = self.queue.pop_front().expect("a queued attempt was seen");
            self.free.remove(&slot);
            self.launch(queued, slot);
        }
    }

    // Starts an attempt: the order is written into its folder and the runtime
    // is told the folder. An attempt that cannot start fails like any other.
    fn launch(&mut self, queued: Queued, slot: usize) {
        let Queued {
            invocation,
            attempt,
            key,
            since,
        } = queued;
        let plan = self.scheduler.plan();
        let task = &plan.tasks[self.scheduler.task_of(invocation)];
        let attempt_id = self.scheduler.attempt_id(invocation, attempt);
        let dir = self.attempt_dir(invocation, attempt);
        let inputs = self.scheduler.read_inputs(invocation);
        let placed = fs::create_dir_all(&dir)
            .and_then(|()| ordered(&dir, attempt_id.clone(), task, inputs))
            .and_then(|order| serde_json::to_vec_pretty(&order).map_err(io::Error::other))
            .and_then(|bytes| fs::write(dir.join(ORDER_FILE), bytes))
            .map_err(|error| error.to_string());
        let began = SystemTime::now();
        let spawned = Instant::now();
        match placed.and_then(|()| self.runtime.start(&dir)) {
            Ok(handle) => {
                self.observer.event(Event::Started {
                    attempt_id: &attempt_id,
                    slot,
                });
                self.out.push(Live {
                    attempt_id,
                    invocation,
                    slot,
                    dir,
                    key,
                    handle,
                    queued: since,
                    spawned,
                    started: None,
                    began,
                    timeout: task.timeout,
                    stopping: None,
                    killed: false,
                });
            }
            Err(reason) => {
                self.free.insert(slot);
                let mut attempt = TaskAttempt::unrun(
                    attempt_id,
                    RunRecord::failed(format!("the task process could not start: {reason}")),
                );
                attempt.began_unix_nanos = Some(unix_nanos(began));
                attempt.slot = u64::try_from(slot).ok();
                attempt.wait_nanos = Some(nanos(spawned.duration_since(since)));
                let decisions = self.scheduler.ended(invocation, attempt, None);
                self.apply(decisions);
            }
        }
    }

    // Notes each attempt that marked itself ready for its body, so its time
    // allowed counts from there and no request to stop reaches it before it
    // can hear one.
    fn notice_started(&mut self, now: Instant) {
        for live in &mut self.out {
            if live.started.is_none() && self.runtime.started(&live.handle) {
                live.started = Some(now);
            }
        }
    }

    // Asks every attempt past its time to stop, and forces every attempt that
    // did not stop within the grace. The time allowed counts from when the
    // body could begin; an attempt that has not begun within the startup
    // allowance is stopped all the same.
    fn stop_due(&mut self, now: Instant) {
        let Timing { startup, grace, .. } = self.timing;
        for live in &mut self.out {
            match live.stopping {
                None => {
                    let due = match live.started {
                        Some(started) => live.timeout.map(|allowed| started + allowed),
                        None => Some(live.spawned + startup),
                    };
                    if due.is_some_and(|due| due <= now) {
                        self.runtime.stop(&live.handle);
                        live.stopping = Some((RunRecord::timed_out(), now));
                    }
                }
                Some((_, since)) => {
                    if !live.killed && since + grace <= now {
                        self.runtime.force(&live.handle);
                        live.killed = true;
                    }
                }
            }
        }
    }

    fn poll_ended(&mut self) -> bool {
        let mut any = false;
        let mut i = 0;
        while i < self.out.len() {
            match self.runtime.poll(&mut self.out[i].handle) {
                None => i += 1,
                Some(ended) => {
                    let live = self.out.swap_remove(i);
                    self.finish(live, ended);
                    any = true;
                }
            }
        }
        any
    }

    // An attempt ended: its record is read back if it left one, the run's
    // authority over its outcome applied, what only the run knows added: the
    // slot, the wait, the runtime's account of it, and the output it left by
    // hash. The record is written whole before the output is kept, so a sweep
    // never finds a kept output that no record names.
    fn finish(&mut self, live: Live<R::TaskHandle>, ended: Ended) {
        let now = Instant::now();
        let Live {
            attempt_id,
            invocation,
            slot,
            dir,
            key,
            queued,
            spawned,
            started,
            began,
            stopping,
            ..
        } = live;
        self.free.insert(slot);
        let record = TaskAttempt::read(&dir.join(RECORD_FILE)).ok();
        // When the body began: as the mark was seen, or, for a process that
        // marked it and ended between two looks, as its own record tells.
        let startup = started
            .map(|started| started.duration_since(spawned))
            .or_else(|| {
                record
                    .as_ref()
                    .and_then(TaskAttempt::began)
                    .and_then(|at| at.duration_since(began).ok())
            });
        let outcome = match (&stopping, &record, &ended.fault) {
            (Some((outcome, _)), _, _) => outcome.clone(),
            (None, Some(attempt), _) => attempt.outcome().clone(),
            (None, None, Some(fault)) => RunRecord::failed(fault.clone()),
            (None, None, None) => RunRecord::failed(no_record(&ended, &dir)),
        };
        let mut attempt = record.unwrap_or_else(|| {
            let mut attempt = TaskAttempt::unrun(attempt_id.clone(), outcome.clone());
            attempt.resources = Some(Resources {
                wall_nanos: nanos(now.duration_since(spawned)),
                ..Resources::default()
            });
            attempt
        });
        attempt.id = Some(attempt_id);
        attempt.outcome = Some(outcome);
        attempt.began_unix_nanos.get_or_insert(unix_nanos(began));
        if stopping.is_some() && startup.is_none() {
            attempt
                .evidence
                .push("stopped before its body began".to_string());
        }
        attempt.slot = u64::try_from(slot).ok();
        attempt.wait_nanos = Some(nanos(spawned.duration_since(queued)));
        attempt.process_record = Some(ProcessRecord {
            program: ended.program,
            args: ended.args,
            wall_nanos: nanos(now.duration_since(spawned)),
            startup_nanos: startup.map(nanos),
            exit_code: ended.exit_code,
            signal: ended.signal,
            killed: stopping.is_some(),
            stdout_bytes: ended.stdout.0,
            stdout_kept: ended.stdout.1,
            stderr_bytes: ended.stderr.0,
            stderr_kept: ended.stderr.1,
            usage: Some(ended.usage),
        });
        let mut output = None;
        if *attempt.outcome() == RunRecord::passed()
            && !self.scheduler.plan().tasks[self.scheduler.task_of(invocation)].leaves_nothing()
        {
            match EncodedValue::from_file(dir.join(OUTPUT_FILE)) {
                Ok(value) => {
                    attempt.output = Some(TaskOutputHash::of(&value.bytes).as_str().to_string());
                    attempt.cache_key = key.map(|key| key.as_str().to_string());
                    output = Some(value);
                }
                Err(error) => {
                    attempt.outcome = Some(RunRecord::failed(format!(
                        "the task process passed but left no output: {error}"
                    )));
                }
            }
        }
        recorded(&dir, &attempt);
        let output = output.and_then(|value| self.kept(&dir, &mut attempt, value));
        let decisions = self.scheduler.ended(invocation, attempt, output);
        self.apply(decisions);
    }

    // The output an attempt left, kept in the store under its hash and named
    // in the cache when the task is cached. An output that cannot be kept
    // fails the attempt; an entry the cache cannot take is noted instead.
    fn kept(
        &self,
        dir: &Path,
        attempt: &mut TaskAttempt,
        mut value: EncodedValue,
    ) -> Option<EncodedValue> {
        let hash = attempt.output_hash()?;
        match self.outputs.keep(&dir.join(OUTPUT_FILE), &hash) {
            Ok(file) => value.file = Some(file),
            Err(error) => {
                attempt.outcome = Some(RunRecord::failed(format!(
                    "the output could not be kept: {error}"
                )));
                attempt.output = None;
                attempt.cache_key = None;
                recorded(dir, attempt);
                return None;
            }
        }
        if let Some(key) = attempt.task_cache_key() {
            if let Err(error) = self.cache.put(&key, &hash) {
                attempt.cache_key = None;
                attempt
                    .evidence
                    .push(format!("the output could not be cached: {error}"));
                recorded(dir, attempt);
            }
        }
        Some(value)
    }
}

// The order for an attempt: which, the task by name, and each read's input,
// a value held in no file yet written into the attempt's folder first.
fn ordered(
    dir: &Path,
    attempt_id: TaskAttemptId,
    task: &Task,
    inputs: Vec<ReadInput>,
) -> io::Result<TaskOrder> {
    let mut handed = Vec::with_capacity(inputs.len());
    for (read_at, (read, input)) in task.reads.iter().zip(inputs).enumerate() {
        let value = match input.value {
            None => None,
            Some(EncodedValue {
                file: Some(file), ..
            }) => Some(file),
            Some(EncodedValue { bytes, file: None }) => {
                let file = dir.join(input_file(read_at));
                fs::write(&file, &bytes)?;
                Some(file)
            }
        };
        handed.push(TaskOrderInput {
            value_type: read.value_type.name.to_string(),
            when: read.when,
            value,
            completion: input.completion,
        });
    }
    Ok(TaskOrder {
        attempt_id,
        task: task.name.clone(),
        inputs: handed,
    })
}

// A key as part of one folder's name: each character a folder name cannot
// hold, or that tells the name's parts apart, written as %XX.
fn folder_safe(key: &str) -> String {
    let mut safe = String::with_capacity(key.len());
    for character in key.chars() {
        if matches!(character, '%' | '/' | '#') || character.is_control() {
            let mut bytes = [0; 4];
            for byte in character.encode_utf8(&mut bytes).bytes() {
                safe.push_str(&format!("%{byte:02X}"));
            }
        } else {
            safe.push(character);
        }
    }
    safe
}

// The attempt's record as the run completed it, over the one its process left.
fn recorded(dir: &Path, attempt: &TaskAttempt) {
    let _ = attempt.write(&dir.join(RECORD_FILE));
}

fn no_record(ended: &Ended, dir: &Path) -> String {
    let how = match (ended.exit_code, ended.signal) {
        (Some(code), _) => format!("exited with code {code}"),
        (None, Some(signal)) => format!("was killed by signal {signal}"),
        (None, None) => "ended".to_string(),
    };
    let said = last_line(&dir.join(STDERR_FILE));
    if said.is_empty() {
        format!("the task process {how} and left no record")
    } else {
        format!("the task process {how} and left no record: {said}")
    }
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

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process;
    use std::time::Duration;
    use std::time::SystemTime;

    use super::folder_safe;
    use super::TaskExecutor;
    use super::Timing;
    use crate::context::stop_token::StopToken;
    use crate::context::task_context::TaskContext;
    use crate::contract::RunId;
    use crate::contract::RunRecord;
    use crate::contract::TaskAttempt;
    use crate::contract::Usage;
    use crate::declaration::plan::Plan;
    use crate::declaration::task::Task;
    use crate::execution::observer::Quiet;
    use crate::execution::run_scheduler::RunScheduler;
    use crate::execution::site::Site;
    use crate::kernel::clock::unix_nanos;
    use crate::protocol::attempt_folder::ORDER_FILE;
    use crate::protocol::attempt_folder::RECORD_FILE;
    use crate::protocol::task_order::TaskOrder;
    use crate::record::refusal::RefusalReason;
    use crate::record::task_failure::TaskFailure;
    use crate::runtime::ended::Ended;
    use crate::runtime::task_runtime::TaskRuntime;
    use crate::value::identity::Identity;
    use crate::value::run_store::RunStore;

    // Attempts that begin their body and end between two looks: the mark is
    // never seen, and each ends at its first poll, its record left as a task
    // process leaves it.
    struct Fleeting;

    impl TaskRuntime for Fleeting {
        type TaskHandle = PathBuf;

        fn verify(&self) -> Vec<RefusalReason> {
            Vec::new()
        }

        fn start(&mut self, dir: &Path) -> Result<PathBuf, String> {
            let order: TaskOrder = fs::read(dir.join(ORDER_FILE))
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .ok_or("no order")?;
            let mut attempt = TaskAttempt::unrun(order.attempt_id, RunRecord::passed());
            attempt.began_unix_nanos = Some(unix_nanos(SystemTime::now()));
            attempt
                .write(&dir.join(RECORD_FILE))
                .map_err(|error| error.to_string())?;
            Ok(dir.to_path_buf())
        }

        fn started(&self, _: &PathBuf) -> bool {
            false
        }

        fn stop(&mut self, _: &PathBuf) {}

        fn force(&mut self, _: &PathBuf) {}

        fn poll(&mut self, _: &mut PathBuf) -> Option<Ended> {
            Some(Ended {
                program: "fleeting".to_string(),
                args: Vec::new(),
                exit_code: Some(0),
                signal: None,
                usage: Usage::default(),
                stdout: (0, 0),
                stderr: (0, 0),
                fault: None,
            })
        }
    }

    #[test]
    fn a_key_names_one_folder_whatever_it_holds() {
        assert_eq!(
            folder_safe("format_rust in libs/a"),
            "format_rust in libs%2Fa"
        );
        assert_eq!(folder_safe("50%#1"), "50%25%231");
        assert_eq!(folder_safe("a\nb"), "a%0Ab");
        assert_eq!(folder_safe("é"), "é");
    }

    #[test]
    fn an_attempt_that_began_and_ended_between_two_looks_keeps_when_its_body_began() {
        let known = [Task::new("blink", |_: &mut TaskContext, (): ()| {
            Ok::<(), TaskFailure>(())
        })];
        let plan = Plan::new("gate", &known, &["blink"]).expect("valid");
        let (scheduler, decisions) =
            RunScheduler::new(&plan, RunStore::new(), Identity::of(b"7"), RunId::next());
        let dir = env::temp_dir().join(format!(
            "task-runner-executor-{}-{}",
            process::id(),
            RunId::next()
        ));
        let site = Site {
            dir: dir.clone(),
            root: dir.clone(),
        };
        let timing = Timing {
            startup: Duration::from_secs(5),
            grace: Duration::from_secs(1),
            tick: Duration::from_millis(1),
        };
        let mut quiet = Quiet;
        let executor =
            TaskExecutor::new(scheduler, decisions, Fleeting, site, 1, timing, &mut quiet);
        let (invocations, _) = executor.execute(&StopToken::new()).records();
        let _ = fs::remove_dir_all(&dir);
        let attempt = invocations[0]
            .attempts
            .last()
            .expect("the attempt is recorded");
        assert_eq!(*attempt.outcome(), RunRecord::passed());
        let process_record = attempt
            .process_record
            .as_ref()
            .expect("the attempt has an account");
        let startup = process_record
            .startup()
            .expect("the record it left tells when its body began");
        assert!(startup <= process_record.wall());
    }
}
