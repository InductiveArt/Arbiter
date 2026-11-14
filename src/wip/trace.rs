use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use serde::Deserialize;
use serde::Serialize;

use crate::wip::job::Cancel;
use crate::wip::job::JobId;
use crate::wip::resources::reap;
use crate::wip::resources::thread_cpu;
use crate::wip::resources::Account;
use crate::wip::resources::Resources;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Outcome {
    Ok,
    Failed(String),
    // Ran past the time its task allowed.
    TimedOut,
    // Stopped from outside before it could finish.
    Cancelled,
    // Never ran: a task it stands on did not pass, named by that task's path.
    Skipped(String),
    // Never ran: the branch it reads was not taken, named by the task that
    // decided it. By design, so it counts against nothing.
    Unneeded(String),
}

impl Outcome {
    pub fn clean(&self) -> bool {
        matches!(self, Outcome::Ok | Outcome::Unneeded(_))
    }

    // An attempt that ended without passing, as opposed to one never made.
    pub fn failed(&self) -> bool {
        matches!(
            self,
            Outcome::Failed(_) | Outcome::TimedOut | Outcome::Cancelled
        )
    }
}

#[derive(Debug)]
pub struct Failure {
    pub reason: String,
}

impl From<String> for Failure {
    fn from(reason: String) -> Self {
        Failure { reason }
    }
}

impl From<&str> for Failure {
    fn from(reason: &str) -> Self {
        Failure {
            reason: reason.to_string(),
        }
    }
}

// How a record relates to the one holding it. An attempt ran as a process of
// its own and is accounted on its own; an inner frame ran inside its parent,
// on the parent's thread, so its figures are already inside the parent's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frame {
    Attempt,
    Inner,
}

// One unit of work that ran, with the units it ran inside itself. The job
// names the run, the path from the root and the attempt, so a unit is found by
// name at any depth and one try is told from the next. Plain data throughout,
// so a record is written and read back whole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub job: JobId,
    pub name: String,
    pub outcome: Outcome,
    pub frame: Frame,
    pub began: Option<SystemTime>,
    // The slot that ran the attempt, and how long it waited for one.
    pub slot: Option<usize>,
    pub wait: Option<Duration>,
    // The product was taken from the store; no process ran.
    pub replayed: bool,
    // What the frame measured of itself, from inside.
    pub resources: Resources,
    // The attempt's own process, as the run that spawned it saw it.
    pub account: Option<Account>,
    // Quantities the body itself counted, by name.
    pub measures: BTreeMap<String, f64>,
    pub evidence: Vec<String>,
    pub children: Vec<Step>,
}

impl Step {
    // The record of a task that never ran.
    pub fn unrun(job: JobId, name: &str, outcome: Outcome) -> Step {
        Step {
            job,
            name: name.to_string(),
            outcome,
            frame: Frame::Attempt,
            began: None,
            slot: None,
            wait: None,
            replayed: false,
            resources: Resources::default(),
            account: None,
            measures: BTreeMap::new(),
            evidence: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn seconds(&self) -> f64 {
        self.resources.wall.as_secs_f64()
    }
}

// Where an attempt lives on disk: its own folder, where its files and any plan
// it runs land, and the store shared by the whole run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub dir: PathBuf,
    pub store: PathBuf,
}

static TERMINATED: AtomicBool = AtomicBool::new(false);

// Whether the host asked this process to stop.
pub fn terminated() -> bool {
    TERMINATED.load(Ordering::Relaxed)
}

// Makes the host's request to stop set a flag instead of ending the process
// at once, so a body stops at its next checkpoint and leaves its record.
#[cfg(unix)]
pub fn stop_on_terminate() {
    extern "C" fn noted(_: libc::c_int) {
        TERMINATED.store(true, Ordering::Relaxed);
    }
    let handler = noted as extern "C" fn(libc::c_int) as libc::sighandler_t;
    unsafe {
        libc::signal(libc::SIGTERM, handler);
    }
}

#[cfg(not(unix))]
pub fn stop_on_terminate() {}

// What a running unit writes through: notes and measures on its own record,
// inner units each measured and nested under it, child processes reaped with
// their account, and the token telling it to stop. Its own clocks start when
// it is made and are read when it finishes.
pub struct Scope {
    job: JobId,
    cancel: Cancel,
    site: Site,
    frame: Frame,
    begun: Instant,
    began: SystemTime,
    cpu_at_start: Option<Duration>,
    measures: BTreeMap<String, f64>,
    spawned: Vec<Account>,
    evidence: Vec<String>,
    children: Vec<Step>,
}

impl Scope {
    pub fn new(job: JobId, cancel: Cancel, site: Site) -> Self {
        Scope::framed(job, cancel, site, Frame::Attempt)
    }

    fn framed(job: JobId, cancel: Cancel, site: Site, frame: Frame) -> Self {
        Scope {
            job,
            cancel,
            site,
            frame,
            begun: Instant::now(),
            began: SystemTime::now(),
            cpu_at_start: thread_cpu(),
            measures: BTreeMap::new(),
            spawned: Vec::new(),
            evidence: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn job(&self) -> &JobId {
        &self.job
    }

    pub fn id(&self) -> &str {
        &self.job.path
    }

    pub fn cancel(&self) -> &Cancel {
        &self.cancel
    }

    pub fn site(&self) -> &Site {
        &self.site
    }

    pub fn dir(&self) -> &Path {
        &self.site.dir
    }

    pub fn note(&mut self, line: impl Into<String>) {
        self.evidence.push(line.into());
    }

    pub fn measure(&mut self, name: impl Into<String>, value: f64) {
        self.measures.insert(name.into(), value);
    }

    // Where a long body checks in, so a job told to stop, by its token or by
    // the host, stops at its next turn instead of running to the end.
    pub fn checkpoint(&self) -> Result<(), Failure> {
        if self.cancel.cancelled() {
            Err("cancelled".into())
        } else if terminated() {
            Err("terminated".into())
        } else {
            Ok(())
        }
    }

    // Runs the command to its end, or kills it the moment the job is told to
    // stop, so a child never outlives the attempt that spawned it. The child
    // is reaped with its account, recorded on this frame whichever way it
    // ended. Both pipes are drained as it runs, so a talkative child never
    // stalls.
    pub fn command(&mut self, command: &mut Command) -> Result<Output, Failure> {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let program = command.get_program().to_string_lossy().into_owned();
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let begun = Instant::now();
        let mut child = command
            .spawn()
            .map_err(|error| Failure::from(error.to_string()))?;
        let stdout = drained(child.stdout.take());
        let stderr = drained(child.stderr.take());
        let mut stopped: Option<Failure> = None;
        let reaped = loop {
            match reap(&mut child, false) {
                Ok(Some(reaped)) => break reaped,
                Ok(None) => match self.checkpoint() {
                    Ok(()) => thread::sleep(Duration::from_millis(5)),
                    Err(failure) => {
                        let _ = child.kill();
                        stopped = Some(failure);
                        match reap(&mut child, true) {
                            Ok(Some(reaped)) => break reaped,
                            Ok(None) => return Err("the killed child could not be reaped".into()),
                            Err(error) => return Err(Failure::from(error.to_string())),
                        }
                    }
                },
                Err(error) => return Err(Failure::from(error.to_string())),
            }
        };
        let stdout = stdout.join().unwrap_or_default();
        let stderr = stderr.join().unwrap_or_default();
        self.spawned.push(Account {
            program,
            args,
            wall: begun.elapsed(),
            startup: None,
            exit_code: reaped.exit_code,
            signal: reaped.signal,
            killed: stopped.is_some(),
            stdout_bytes: stdout.len() as u64,
            stdout_kept: stdout.len() as u64,
            stderr_bytes: stderr.len() as u64,
            stderr_kept: stderr.len() as u64,
            usage: reaped.usage,
        });
        if let Some(failure) = stopped {
            return Err(failure);
        }
        Ok(Output {
            status: reaped.status,
            stdout,
            stderr,
        })
    }

    // A failing inner unit is recorded and its failure still returned, so the
    // caller decides whether the whole falls with it.
    pub fn step<T>(
        &mut self,
        name: &str,
        work: impl FnOnce(&mut Scope) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let inner_job = JobId::new(
            self.job.run,
            format!("{}/{name}", self.job.path),
            self.job.attempt,
        );
        let mut inner = Scope::framed(
            inner_job,
            self.cancel.clone(),
            self.site.clone(),
            Frame::Inner,
        );
        let result = work(&mut inner);
        let outcome = match &result {
            Ok(_) => Outcome::Ok,
            Err(failure) => Outcome::Failed(failure.reason.clone()),
        };
        self.children.push(inner.finished(name, outcome));
        result
    }

    pub fn attach(&mut self, step: Step) {
        self.children.push(step);
    }

    pub fn finished(self, name: &str, outcome: Outcome) -> Step {
        let wall = self.begun.elapsed();
        let cpu = self
            .cpu_at_start
            .and_then(|start| thread_cpu().map(|now| now.saturating_sub(start)));
        Step {
            job: self.job,
            name: name.to_string(),
            outcome,
            frame: self.frame,
            began: Some(self.began),
            slot: None,
            wait: None,
            replayed: false,
            resources: Resources {
                wall,
                cpu,
                children: self.spawned,
            },
            account: None,
            measures: self.measures,
            evidence: self.evidence,
            children: self.children,
        }
    }
}

fn drained(pipe: Option<impl Read + Send + 'static>) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    })
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::hint::black_box;
    use std::process::Command;
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;

    use super::Failure;
    use super::Frame;
    use super::Outcome;
    use super::Scope;
    use super::Site;
    use super::Step;
    use crate::wip::job::Cancel;
    use crate::wip::job::JobId;
    use crate::wip::job::RunId;

    fn site() -> Site {
        Site {
            dir: env::temp_dir(),
            store: env::temp_dir(),
        }
    }

    fn scope(path: &str) -> Scope {
        Scope::new(JobId::new(RunId::next(), path, 1), Cancel::new(), site())
    }

    #[test]
    fn an_inner_step_nests_under_its_parent_with_a_path_id() {
        let mut scope = scope("gate/tools");
        let answer = scope.step("clippy", |inner| {
            inner.note("0 warnings");
            Ok::<u8, Failure>(7)
        });
        assert_eq!(answer.ok(), Some(7));
        let step = scope.finished("tools", Outcome::Ok);
        assert_eq!(step.children.len(), 1);
        assert_eq!(step.children[0].job.path, "gate/tools/clippy");
        assert_eq!(step.children[0].job.attempt, 1);
        assert_eq!(step.children[0].evidence, ["0 warnings"]);
    }

    #[test]
    fn a_failing_inner_step_is_recorded_and_its_failure_returned() {
        let mut scope = scope("gate/tools");
        let answer: Result<(), Failure> = scope.step("test", |_| Err("2 tests failed".into()));
        assert_eq!(
            answer.err().map(|failure| failure.reason),
            Some("2 tests failed".to_string())
        );
        let step = scope.finished("tools", Outcome::Ok);
        assert_eq!(
            step.children[0].outcome,
            Outcome::Failed("2 tests failed".to_string())
        );
    }

    #[test]
    fn an_inner_step_is_an_inner_frame_counted_inside_its_parent() {
        let mut scope = scope("gate/tools");
        scope
            .step("clippy", |_| {
                thread::sleep(Duration::from_millis(30));
                Ok::<(), Failure>(())
            })
            .expect("ok");
        let step = scope.finished("tools", Outcome::Ok);
        assert_eq!(step.frame, Frame::Attempt);
        assert_eq!(step.children[0].frame, Frame::Inner);
        assert!(step.children[0].seconds() >= 0.03);
        assert!(step.seconds() >= step.children[0].seconds());
    }

    #[test]
    fn a_frame_records_when_it_began_and_how_long_it_ran() {
        let before = std::time::SystemTime::now();
        let scope = scope("gate/scan");
        thread::sleep(Duration::from_millis(20));
        let step = scope.finished("scan", Outcome::Ok);
        let began = step.began.expect("an attempt records when it began");
        assert!(began >= before);
        assert!(step.seconds() >= 0.02);
        assert_eq!((step.slot, step.wait, step.account), (None, None, None));
    }

    #[test]
    fn a_frame_is_charged_the_cpu_time_its_thread_spends() {
        let scope = scope("gate/scan");
        let until = Instant::now() + Duration::from_millis(40);
        let mut x: u64 = 1;
        while Instant::now() < until {
            x = black_box(
                x.wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407),
            );
        }
        let step = scope.finished("scan", Outcome::Ok);
        let cpu = step
            .resources
            .cpu
            .expect("thread CPU time is measured on this host");
        assert!(
            cpu >= Duration::from_millis(10),
            "spun for 40ms of wall but {cpu:?} of CPU"
        );
    }

    #[test]
    fn a_sleeping_frame_spends_wall_but_little_cpu() {
        let scope = scope("gate/scan");
        thread::sleep(Duration::from_millis(40));
        let step = scope.finished("scan", Outcome::Ok);
        let cpu = step.resources.cpu.expect("measured");
        assert!(step.seconds() >= 0.04);
        assert!(
            cpu < Duration::from_millis(20),
            "sleeping cost {cpu:?} of CPU"
        );
    }

    #[test]
    fn a_measure_is_recorded_by_name() {
        let mut scope = scope("gate/scan");
        scope.measure("files", 41.0);
        scope.measure("files", 42.0);
        scope.measure("bytes", 1024.0);
        let step = scope.finished("scan", Outcome::Ok);
        assert_eq!(step.measures.get("files"), Some(&42.0));
        assert_eq!(step.measures.get("bytes"), Some(&1024.0));
        assert_eq!(step.measures.len(), 2);
    }

    #[test]
    fn a_record_written_as_json_reads_back_equal() {
        let mut scope = scope("gate/tools");
        scope.note("noted");
        scope.measure("files", 3.0);
        scope
            .command(Command::new("echo").arg("hi"))
            .expect("echo runs");
        scope.step("clippy", |_| Ok::<(), Failure>(())).expect("ok");
        let step = scope.finished("tools", Outcome::Failed("one warning".to_string()));
        let json = serde_json::to_vec_pretty(&step).expect("a record encodes");
        let back: Step = serde_json::from_slice(&json).expect("a record decodes");
        assert_eq!(back, step);
    }

    #[test]
    fn an_outcome_is_clean_when_it_passed_or_was_not_needed() {
        assert!(Outcome::Ok.clean());
        assert!(Outcome::Unneeded("gate/check".to_string()).clean());
        assert!(!Outcome::Failed("broke".to_string()).clean());
        assert!(!Outcome::TimedOut.clean());
        assert!(!Outcome::Cancelled.clean());
        assert!(!Outcome::Skipped("gate/check".to_string()).clean());
    }

    #[test]
    fn an_outcome_failed_when_an_attempt_ended_without_passing() {
        assert!(Outcome::Failed("broke".to_string()).failed());
        assert!(Outcome::TimedOut.failed());
        assert!(Outcome::Cancelled.failed());
        assert!(!Outcome::Ok.failed());
        assert!(!Outcome::Skipped("gate/check".to_string()).failed());
        assert!(!Outcome::Unneeded("gate/check".to_string()).failed());
    }

    #[test]
    fn a_checkpoint_fails_once_the_job_is_cancelled() {
        let cancel = Cancel::new();
        let scope = Scope::new(
            JobId::new(RunId::next(), "gate/laws", 1),
            cancel.clone(),
            site(),
        );
        assert!(scope.checkpoint().is_ok());
        cancel.cancel();
        assert_eq!(
            scope.checkpoint().err().map(|failure| failure.reason),
            Some("cancelled".to_string())
        );
    }

    #[test]
    fn a_command_runs_to_its_end_with_its_output_read() {
        let mut scope = scope("gate/tools");
        let output = scope
            .command(Command::new("echo").arg("hi"))
            .expect("echo runs");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "hi\n");
    }

    #[test]
    fn a_command_s_account_is_recorded_on_the_frame_that_ran_it() {
        let mut scope = scope("gate/tools");
        scope
            .command(Command::new("echo").arg("hi"))
            .expect("echo runs");
        let step = scope.finished("tools", Outcome::Ok);
        assert_eq!(step.resources.children.len(), 1);
        let child = &step.resources.children[0];
        assert_eq!(child.program, "echo");
        assert_eq!(child.args, ["hi"]);
        assert_eq!(
            (child.exit_code, child.signal, child.killed),
            (Some(0), None, false)
        );
        assert_eq!((child.stdout_bytes, child.stdout_kept), (3, 3));
        assert!(
            child.usage.max_rss_bytes > 0,
            "a reaped child reports its peak memory"
        );
        assert!(child.wall > Duration::ZERO);
    }

    #[test]
    fn a_command_is_killed_once_the_job_is_cancelled() {
        let cancel = Cancel::new();
        let mut scope = Scope::new(
            JobId::new(RunId::next(), "gate/tools", 1),
            cancel.clone(),
            site(),
        );
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            cancel.cancel();
        });
        let begun = Instant::now();
        let result = scope.command(Command::new("sleep").arg("5"));
        assert_eq!(
            result.err().map(|failure| failure.reason),
            Some("cancelled".to_string())
        );
        assert!(
            begun.elapsed() < Duration::from_secs(2),
            "the child outlived the cancellation"
        );
        let step = scope.finished("tools", Outcome::Cancelled);
        let child = &step.resources.children[0];
        assert!(child.killed);
        assert_eq!(child.exit_code, None);
        assert_eq!(child.signal, Some(9));
    }
}
