use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use crate::job::cancel::Cancel;
use crate::job::id::JobId;
use crate::kernel::child::reap;
use crate::kernel::clock::thread_cpu;
use crate::kernel::signal::terminated;
use crate::layout::Site;
use crate::trace::account::Account;
use crate::trace::account::Resources;
use crate::trace::outcome::Failure;
use crate::trace::outcome::Outcome;
use crate::trace::step::Frame;
use crate::trace::step::Step;

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
    // Plans run inside this attempt so far, by any of its frames: a step
    // starts from its parent's count and hands its own back when it ends.
    runs: u32,
    // The identity of the code the run around this attempt keys its products
    // by; none for an attempt made outside any run.
    identity: Option<u64>,
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
            runs: 0,
            identity: None,
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
            .map_err(|error| Failure::from(format!("{program} could not be started: {error}")))?;
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
        inner.runs = self.runs;
        inner.identity = self.identity;
        let result = work(&mut inner);
        self.runs = inner.runs;
        let outcome = match &result {
            Ok(_) => Outcome::Ok,
            Err(failure) => Outcome::Failed(failure.reason.clone()),
        };
        self.children.push(inner.finished(name, outcome));
        result
    }

    // An attempt the run around it hands the identity of its code, so a plan
    // run inside it keys its products by the same.
    pub(crate) fn identified(mut self, identity: u64) -> Scope {
        self.identity = Some(identity);
        self
    }

    pub(crate) fn identity(&self) -> Option<u64> {
        self.identity
    }

    // The number the next plan run inside this attempt goes by, so no two
    // share a folder or an id.
    pub(crate) fn next_run(&mut self) -> u32 {
        self.runs += 1;
        self.runs
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

    use super::Scope;
    use crate::job::cancel::Cancel;
    use crate::job::id::JobId;
    use crate::job::id::RunId;
    use crate::layout::Site;
    use crate::trace::outcome::Failure;
    use crate::trace::outcome::Outcome;
    use crate::trace::step::Frame;
    use crate::trace::step::Step;

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
    fn a_command_whose_program_cannot_be_started_fails_naming_the_program() {
        let mut scope = scope("gate/tools");
        let failed = scope
            .command(&mut Command::new("task-runner-no-such-program"))
            .expect_err("nothing ran");
        assert!(
            failed
                .reason
                .starts_with("task-runner-no-such-program could not be started: "),
            "the reason names the program: {}",
            failed.reason
        );
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
