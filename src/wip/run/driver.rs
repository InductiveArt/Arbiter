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

use crate::wip::job::cancel::Cancel;
use crate::wip::job::id::JobId;
use crate::wip::kernel::signal::terminated;
use crate::wip::layout::Site;
use crate::wip::layout::KEY_FILE;
use crate::wip::layout::ORDER_FILE;
use crate::wip::layout::PRODUCT_FILE;
use crate::wip::layout::RECORD_FILE;
use crate::wip::layout::STDERR_FILE;
use crate::wip::run::flow::Decision;
use crate::wip::run::flow::Flow;
use crate::wip::run::medium::Ended;
use crate::wip::run::medium::Medium;
use crate::wip::run::observer::Event;
use crate::wip::run::observer::Observer;
use crate::wip::store::Product;
use crate::wip::trace::account::Account;
use crate::wip::trace::outcome::Outcome;
use crate::wip::trace::step::Step;
use crate::wip::worker::Input;
use crate::wip::worker::Order;

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
    at: usize,
    attempt: u32,
    key: Option<String>,
    since: Instant,
}

// An attempt out in the medium: which task and slot, its folder, the key its
// product is stored under, when it was queued and started, when its body could
// begin, how long it may take from then, and whether it has been told to stop.
struct Live<H> {
    id: JobId,
    at: usize,
    slot: usize,
    dir: PathBuf,
    key: Option<String>,
    handle: H,
    queued: Instant,
    spawned: Instant,
    started: Option<Instant>,
    began: SystemTime,
    timeout: Option<Duration>,
    stopping: Option<(Outcome, Instant)>,
    killed: bool,
}

// Drives one run: takes the flow's decisions, hands attempts to the medium up
// to the slots, keeps the only clock, reads back what each attempt left and
// feeds the flow what ended, telling the observer each event as it happens.
pub struct Driver<'a, 'o, M: Medium> {
    flow: Flow<'a>,
    medium: M,
    site: Site,
    timing: Timing,
    free: BTreeSet<usize>,
    queue: VecDeque<Queued>,
    out: Vec<Live<M::Handle>>,
    observer: &'o mut dyn Observer,
    cancelled: bool,
}

impl<'a, 'o, M: Medium> Driver<'a, 'o, M> {
    pub fn new(
        flow: Flow<'a>,
        decisions: Vec<Decision>,
        medium: M,
        site: Site,
        slots: usize,
        timing: Timing,
        observer: &'o mut dyn Observer,
    ) -> Driver<'a, 'o, M> {
        let mut driver = Driver {
            flow,
            medium,
            site,
            timing,
            free: (0..slots.max(1)).collect(),
            queue: VecDeque::new(),
            out: Vec::new(),
            observer,
            cancelled: false,
        };
        driver.apply(decisions);
        driver
    }

    // Runs until every task settled, or the token or a signal cancels the run
    // and what the cancel still unlocks has settled too.
    pub fn drive(mut self, cancel: &Cancel) -> Flow<'a> {
        loop {
            if self.flow.settled() {
                break;
            }
            if !self.cancelled && (cancel.cancelled() || terminated()) {
                self.cancelled = true;
                let decisions = self.flow.cancel();
                self.apply(decisions);
                continue;
            }
            self.fill();
            let now = Instant::now();
            self.notice_started(now);
            self.stop_due(now);
            if self.poll_ended() || self.flow.settled() {
                continue;
            }
            assert!(!self.out.is_empty(), "a valid plan settles every task");
            thread::sleep(self.timing.tick);
        }
        self.flow
    }

    fn attempt_dir(&self, at: usize, attempt: u32) -> PathBuf {
        let plan = self.flow.plan();
        self.site
            .dir
            .join(&plan.name)
            .join(format!("{}#{attempt}", plan.tasks[at].name))
    }

    // Decisions are taken in the order made, those a decision leads to before
    // the ones after it, so a replay's readers queue where the replayed task
    // would have.
    fn apply(&mut self, decisions: Vec<Decision>) {
        let mut pending: VecDeque<Decision> = decisions.into();
        while let Some(decision) = pending.pop_front() {
            let more = match decision {
                Decision::Attempt { at, attempt, key } => self.wanted(at, attempt, key),
                Decision::Stop { at } => self.stop(at),
                Decision::Recorded { at, step } => {
                    if step.replayed {
                        let dir = self.attempt_dir(at, 0);
                        if let Ok(bytes) = serde_json::to_vec_pretty(&step) {
                            let _ = fs::write(dir.join(RECORD_FILE), bytes);
                        }
                    }
                    self.observer.event(Event::Ended { step: &step });
                    Vec::new()
                }
                Decision::Settled { at, outcome } => {
                    let path = self.flow.job(at, 0).path;
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

    // A product already in the store under the key is replayed, no process
    // runs; otherwise the attempt waits for a slot. The key is written first,
    // so a sweep never sees a stored product that nothing refers to.
    fn wanted(&mut self, at: usize, attempt: u32, key: Option<String>) -> Vec<Decision> {
        if let Some(key) = &key {
            if let Ok(product) = Product::from_file(self.site.store.join(key)) {
                let dir = self.attempt_dir(at, 0);
                let _ = fs::create_dir_all(&dir);
                let _ = fs::write(dir.join(KEY_FILE), key);
                return self.flow.replayed(at, product);
            }
        }
        self.queue.push_back(Queued {
            at,
            attempt,
            key,
            since: Instant::now(),
        });
        Vec::new()
    }

    // The task's attempt is told to stop if it is out, and ends as cancelled
    // without an attempt if it was still waiting for a slot.
    fn stop(&mut self, at: usize) -> Vec<Decision> {
        if let Some(live) = self.out.iter_mut().find(|live| live.at == at) {
            if live.stopping.is_none() {
                self.medium.stop(&live.handle);
                live.stopping = Some((Outcome::Cancelled, Instant::now()));
            }
            return Vec::new();
        }
        if let Some(waiting) = self.queue.iter().position(|queued| queued.at == at) {
            self.queue.remove(waiting);
            return self.flow.withdrawn(at);
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

    // Starts an attempt: the order is written into its folder and the medium
    // is told the folder. An attempt that cannot start fails like any other.
    fn launch(&mut self, queued: Queued, slot: usize) {
        let Queued {
            at,
            attempt,
            key,
            since,
        } = queued;
        let plan = self.flow.plan();
        let task = &plan.tasks[at];
        let id = self.flow.job(at, attempt);
        let dir = self.attempt_dir(at, attempt);
        let order = Order {
            id: id.clone(),
            task: task.name.clone(),
            store: self.site.store.clone(),
            identity: self.flow.identity(),
            inputs: task
                .reads
                .iter()
                .zip(self.flow.parts(at))
                .map(|(read, part)| Input {
                    need: read.need.name.to_string(),
                    when: read.when,
                    value: part.value.and_then(|product| product.file),
                    completion: part.completion,
                })
                .collect(),
        };
        let placed = fs::create_dir_all(&dir)
            .and_then(|()| serde_json::to_vec_pretty(&order).map_err(io::Error::other))
            .and_then(|bytes| fs::write(dir.join(ORDER_FILE), bytes))
            .map_err(|error| error.to_string());
        let began = SystemTime::now();
        let spawned = Instant::now();
        match placed.and_then(|()| self.medium.start(&dir)) {
            Ok(handle) => {
                self.observer.event(Event::Started { job: &id, slot });
                self.out.push(Live {
                    id,
                    at,
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
                let mut step = Step::unrun(
                    id,
                    &task.name,
                    Outcome::Failed(format!("the worker could not start: {reason}")),
                );
                step.began = Some(began);
                step.slot = Some(slot);
                step.wait = Some(spawned.duration_since(since));
                let decisions = self.flow.ended(at, step, None);
                self.apply(decisions);
            }
        }
    }

    // Notes each attempt that marked itself ready for its body, so its time
    // allowed counts from there and no request to stop reaches it before it
    // can hear one.
    fn notice_started(&mut self, now: Instant) {
        for live in &mut self.out {
            if live.started.is_none() && self.medium.started(&live.handle) {
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
                        self.medium.stop(&live.handle);
                        live.stopping = Some((Outcome::TimedOut, now));
                    }
                }
                Some((_, since)) => {
                    if !live.killed && since + grace <= now {
                        self.medium.force(&live.handle);
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
            match self.medium.poll(&mut self.out[i].handle) {
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
    // authority over its outcome applied, and what only the run knows added:
    // the slot, the wait, and the medium's account of it.
    fn finish(&mut self, live: Live<M::Handle>, ended: Ended) {
        let now = Instant::now();
        let Live {
            id,
            at,
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
        let name = self.flow.plan().tasks[at].name.clone();
        let record: Option<Step> = fs::read(dir.join(RECORD_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let mut outcome = match (&stopping, &record, &ended.fault) {
            (Some((outcome, _)), _, _) => outcome.clone(),
            (None, Some(step), _) => step.outcome.clone(),
            (None, None, Some(fault)) => Outcome::Failed(fault.clone()),
            (None, None, None) => Outcome::Failed(no_record(&ended, &dir)),
        };
        let value = if outcome == Outcome::Ok {
            match product(&dir, key.as_deref(), &self.site.store) {
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
            let mut step = Step::unrun(id.clone(), &name, outcome.clone());
            step.resources.wall = now.duration_since(spawned);
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
            program: ended.program,
            args: ended.args,
            wall: now.duration_since(spawned),
            startup: started.map(|started| started.duration_since(spawned)),
            exit_code: ended.exit_code,
            signal: ended.signal,
            killed: stopping.is_some(),
            stdout_bytes: ended.stdout.0,
            stdout_kept: ended.stdout.1,
            stderr_bytes: ended.stderr.0,
            stderr_kept: ended.stderr.1,
            usage: ended.usage,
        });
        let decisions = self.flow.ended(at, step, value);
        self.apply(decisions);
    }
}

// The product an attempt left, moved into the store under its key when the
// task is cached. The key is written first, so a sweep never sees a stored
// product that nothing refers to yet.
fn product(dir: &Path, key: Option<&str>, store: &Path) -> Result<Product, String> {
    let file = dir.join(PRODUCT_FILE);
    let mut product = Product::from_file(file.clone())
        .map_err(|error| format!("the worker passed but left no product: {error}"))?;
    if let Some(key) = key {
        let kept = store.join(key);
        let stored = fs::create_dir_all(store)
            .and_then(|()| fs::write(dir.join(KEY_FILE), key))
            .and_then(|()| fs::rename(&file, &kept));
        if stored.is_ok() {
            product.file = Some(kept);
        }
    }
    Ok(product)
}

fn no_record(ended: &Ended, dir: &Path) -> String {
    let how = match (ended.exit_code, ended.signal) {
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
