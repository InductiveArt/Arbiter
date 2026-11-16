use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use crate::context::stop_token::StopToken;
use crate::contract::ProcessRecord;
use crate::contract::Resources;
use crate::contract::RunRecord;
use crate::contract::TaskAttempt;
use crate::contract::TaskAttemptId;
use crate::contract::TaskSubstep;
use crate::kernel::child::reap;
use crate::kernel::clock::nanos;
use crate::kernel::clock::thread_cpu;
use crate::kernel::clock::unix_nanos;
use crate::kernel::signal::terminated;
use crate::record::task_failure::TaskFailure;

// What a body writes through, for its attempt or for one of its substeps:
// notes and measures on its own record, substeps each measured and nested
// under it, child processes reaped with their account, and the token telling
// it to stop. Its own clocks start when it is made and are read when it
// finishes.
pub struct TaskContext {
    attempt_id: TaskAttemptId,
    stop_token: StopToken,
    dir: PathBuf,
    begun: Instant,
    began: SystemTime,
    cpu_at_start: Option<Duration>,
    measures: BTreeMap<String, f64>,
    processes: Vec<ProcessRecord>,
    evidence: Vec<String>,
    substeps: Vec<TaskSubstep>,
}

impl TaskContext {
    // A context for the attempt, working in the attempt's own folder.
    pub fn new(attempt_id: TaskAttemptId, stop_token: StopToken, dir: PathBuf) -> Self {
        TaskContext {
            attempt_id,
            stop_token,
            dir,
            begun: Instant::now(),
            began: SystemTime::now(),
            cpu_at_start: thread_cpu(),
            measures: BTreeMap::new(),
            processes: Vec::new(),
            evidence: Vec::new(),
            substeps: Vec::new(),
        }
    }

    pub fn attempt_id(&self) -> &TaskAttemptId {
        &self.attempt_id
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn note(&mut self, line: impl Into<String>) {
        self.evidence.push(line.into());
    }

    pub fn measure(&mut self, name: impl Into<String>, value: f64) {
        self.measures.insert(name.into(), value);
    }

    // Where a long body checks in, so an attempt told to stop, by its token or
    // by the host, stops at its next turn instead of running to the end.
    pub fn checkpoint(&self) -> Result<(), TaskFailure> {
        if self.stop_token.stopped() {
            Err("cancelled".into())
        } else if terminated() {
            Err("terminated".into())
        } else {
            Ok(())
        }
    }

    // Runs the command to its end, or kills it the moment the attempt is told
    // to stop, so a child never outlives the attempt that spawned it. The child
    // is reaped with its account, recorded on this context whichever way it
    // ended. Both pipes are drained as it runs, so a talkative child never
    // stalls.
    pub fn command(&mut self, command: &mut Command) -> Result<Output, TaskFailure> {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let program = command.get_program().to_string_lossy().into_owned();
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let begun = Instant::now();
        let mut child = command.spawn().map_err(|error| {
            TaskFailure::from(format!("{program} could not be started: {error}"))
        })?;
        let stdout = drained(child.stdout.take());
        let stderr = drained(child.stderr.take());
        let mut stopped: Option<TaskFailure> = None;
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
                            Err(error) => return Err(TaskFailure::from(error.to_string())),
                        }
                    }
                },
                Err(error) => return Err(TaskFailure::from(error.to_string())),
            }
        };
        let stdout = stdout.join().unwrap_or_default();
        let stderr = stderr.join().unwrap_or_default();
        self.processes.push(ProcessRecord {
            program,
            args,
            wall_nanos: nanos(begun.elapsed()),
            startup_nanos: None,
            exit_code: reaped.exit_code,
            signal: reaped.signal,
            killed: stopped.is_some(),
            stdout_bytes: stdout.len() as u64,
            stdout_kept: stdout.len() as u64,
            stderr_bytes: stderr.len() as u64,
            stderr_kept: stderr.len() as u64,
            usage: Some(reaped.usage),
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

    // A failing substep is recorded and its failure still returned, so the
    // caller decides whether the whole falls with it.
    pub fn substep<T>(
        &mut self,
        name: &str,
        work: impl FnOnce(&mut TaskContext) -> Result<T, TaskFailure>,
    ) -> Result<T, TaskFailure> {
        let mut inner = TaskContext::new(
            self.attempt_id.clone(),
            self.stop_token.clone(),
            self.dir.clone(),
        );
        let result = work(&mut inner);
        let outcome = match &result {
            Ok(_) => RunRecord::passed(),
            Err(failure) => RunRecord::failed(failure.reason.clone()),
        };
        self.substeps.push(inner.finished_substep(name, outcome));
        result
    }

    pub fn finished(self, outcome: RunRecord) -> TaskAttempt {
        let (wall, cpu) = self.clocks();
        TaskAttempt {
            id: Some(self.attempt_id),
            outcome: Some(outcome),
            began_unix_nanos: Some(unix_nanos(self.began)),
            slot: None,
            wait_nanos: None,
            replayed: false,
            output: None,
            cache_key: None,
            resources: Some(Resources {
                wall_nanos: nanos(wall),
                cpu_nanos: cpu.map(nanos),
                processes: self.processes,
            }),
            process_record: None,
            measures: self.measures,
            evidence: self.evidence,
            substeps: self.substeps,
        }
    }

    fn finished_substep(self, name: &str, outcome: RunRecord) -> TaskSubstep {
        let (wall, cpu) = self.clocks();
        TaskSubstep {
            name: name.to_string(),
            outcome: Some(outcome),
            began_unix_nanos: Some(unix_nanos(self.began)),
            resources: Some(Resources {
                wall_nanos: nanos(wall),
                cpu_nanos: cpu.map(nanos),
                processes: self.processes,
            }),
            measures: self.measures,
            evidence: self.evidence,
            substeps: self.substeps,
        }
    }

    // The wall since this context was made, and the CPU time its thread spent
    // since then when the host can tell.
    fn clocks(&self) -> (Duration, Option<Duration>) {
        let cpu = self
            .cpu_at_start
            .and_then(|start| thread_cpu().map(|now| now.saturating_sub(start)));
        (self.begun.elapsed(), cpu)
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

    use super::TaskContext;
    use crate::context::stop_token::StopToken;
    use crate::contract::RunId;
    use crate::contract::RunRecord;
    use crate::contract::TaskAttempt;
    use crate::contract::TaskAttemptId;
    use crate::kernel::clock::thread_cpu;
    use crate::record::task_failure::TaskFailure;

    fn context(path: &str) -> TaskContext {
        TaskContext::new(
            TaskAttemptId::new(RunId::next(), path, 1),
            StopToken::new(),
            env::temp_dir(),
        )
    }

    #[test]
    fn a_substep_nests_under_its_attempt_by_name() {
        let mut context = context("gate/tools");
        let answer = context.substep("clippy", |substep| {
            substep.note("0 warnings");
            Ok::<u8, TaskFailure>(7)
        });
        assert_eq!(answer.ok(), Some(7));
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(attempt.substeps.len(), 1);
        assert_eq!(attempt.substeps[0].name, "clippy");
        assert_eq!(attempt.substeps[0].evidence, ["0 warnings"]);
    }

    #[test]
    fn a_failing_substep_is_recorded_and_its_failure_returned() {
        let mut context = context("gate/tools");
        let answer: Result<(), TaskFailure> =
            context.substep("test", |_| Err("2 tests failed".into()));
        assert_eq!(
            answer.err().map(|failure| failure.reason),
            Some("2 tests failed".to_string())
        );
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(
            *attempt.substeps[0].outcome(),
            RunRecord::failed("2 tests failed".to_string())
        );
    }

    #[test]
    fn a_substep_is_counted_inside_its_attempt() {
        let mut context = context("gate/tools");
        context
            .substep("clippy", |_| {
                thread::sleep(Duration::from_millis(30));
                Ok::<(), TaskFailure>(())
            })
            .expect("ok");
        let attempt = context.finished(RunRecord::passed());
        assert!(attempt.substeps[0].seconds() >= 0.03);
        assert!(attempt.seconds() >= attempt.substeps[0].seconds());
    }

    #[test]
    fn an_attempt_records_when_it_began_and_how_long_it_ran() {
        let before = std::time::SystemTime::now();
        let context = context("gate/scan");
        thread::sleep(Duration::from_millis(20));
        let attempt = context.finished(RunRecord::passed());
        let began = attempt.began().expect("an attempt records when it began");
        assert!(began >= before);
        assert!(attempt.seconds() >= 0.02);
        assert_eq!(
            (attempt.slot, attempt.wait(), attempt.process_record),
            (None, None, None)
        );
    }

    #[test]
    fn an_attempt_is_charged_the_cpu_time_its_thread_spends() {
        let context = context("gate/scan");
        let before = thread_cpu().expect("thread CPU time is measured on this host");
        // Computes until the thread has had 20ms of CPU, however long a loaded
        // machine takes to give them, within five seconds of wall.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut x: u64 = 1;
        while thread_cpu().is_some_and(|now| now.saturating_sub(before) < Duration::from_millis(20))
            && Instant::now() < deadline
        {
            for _ in 0..10_000 {
                x = black_box(
                    x.wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407),
                );
            }
        }
        let attempt = context.finished(RunRecord::passed());
        let cpu = attempt
            .resources()
            .cpu()
            .expect("thread CPU time is measured on this host");
        assert!(
            cpu >= Duration::from_millis(20),
            "the thread had 20ms of CPU but its attempt was charged {cpu:?}"
        );
    }

    #[test]
    fn a_sleeping_attempt_spends_wall_but_little_cpu() {
        let context = context("gate/scan");
        thread::sleep(Duration::from_millis(40));
        let attempt = context.finished(RunRecord::passed());
        let cpu = attempt.resources().cpu().expect("measured");
        assert!(attempt.seconds() >= 0.04);
        assert!(
            cpu < Duration::from_millis(20),
            "sleeping cost {cpu:?} of CPU"
        );
    }

    #[test]
    fn a_measure_is_recorded_by_name() {
        let mut context = context("gate/scan");
        context.measure("files", 41.0);
        context.measure("files", 42.0);
        context.measure("bytes", 1024.0);
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(attempt.measures.get("files"), Some(&42.0));
        assert_eq!(attempt.measures.get("bytes"), Some(&1024.0));
        assert_eq!(attempt.measures.len(), 2);
    }

    #[test]
    fn a_record_written_as_json_reads_back_equal() {
        let mut context = context("gate/tools");
        context.note("noted");
        context.measure("files", 3.0);
        context
            .command(Command::new("echo").arg("hi"))
            .expect("echo runs");
        context
            .substep("clippy", |_| Ok::<(), TaskFailure>(()))
            .expect("ok");
        let attempt = context.finished(RunRecord::failed("one warning".to_string()));
        let file = env::temp_dir().join(format!(
            "task-runner-context-{}-{}.json",
            std::process::id(),
            RunId::next()
        ));
        attempt.write(&file).expect("the record is written");
        let back = TaskAttempt::read(&file).expect("the record is read");
        let _ = std::fs::remove_file(&file);
        assert_eq!(back, attempt);
    }

    #[test]
    fn a_checkpoint_fails_once_the_attempt_is_told_to_stop() {
        let token = StopToken::new();
        let context = TaskContext::new(
            TaskAttemptId::new(RunId::next(), "gate/laws", 1),
            token.clone(),
            env::temp_dir(),
        );
        assert!(context.checkpoint().is_ok());
        token.stop();
        assert_eq!(
            context.checkpoint().err().map(|failure| failure.reason),
            Some("cancelled".to_string())
        );
    }

    #[test]
    fn a_command_runs_to_its_end_with_its_output_read() {
        let mut context = context("gate/tools");
        let output = context
            .command(Command::new("echo").arg("hi"))
            .expect("echo runs");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "hi\n");
    }

    #[test]
    fn a_command_whose_program_cannot_be_started_fails_naming_the_program() {
        let mut context = context("gate/tools");
        let failed = context
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
    fn a_command_s_account_is_recorded_on_the_context_that_ran_it() {
        let mut context = context("gate/tools");
        context
            .command(Command::new("echo").arg("hi"))
            .expect("echo runs");
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(attempt.resources().processes.len(), 1);
        let child = &attempt.resources().processes[0];
        assert_eq!(child.program, "echo");
        assert_eq!(child.args, ["hi"]);
        assert_eq!(
            (child.exit_code, child.signal, child.killed),
            (Some(0), None, false)
        );
        assert_eq!((child.stdout_bytes, child.stdout_kept), (3, 3));
        assert!(
            child.usage().max_rss_bytes > 0,
            "a reaped child reports its peak memory"
        );
        assert!(child.wall() > Duration::ZERO);
    }

    #[test]
    fn a_command_is_killed_once_the_attempt_is_told_to_stop() {
        let token = StopToken::new();
        let mut context = TaskContext::new(
            TaskAttemptId::new(RunId::next(), "gate/tools", 1),
            token.clone(),
            env::temp_dir(),
        );
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            token.stop();
        });
        let begun = Instant::now();
        let result = context.command(Command::new("sleep").arg("5"));
        assert_eq!(
            result.err().map(|failure| failure.reason),
            Some("cancelled".to_string())
        );
        assert!(
            begun.elapsed() < Duration::from_secs(2),
            "the child outlived the cancellation"
        );
        let attempt = context.finished(RunRecord::cancelled());
        let child = &attempt.resources().processes[0];
        assert!(child.killed);
        assert_eq!(child.exit_code, None);
        assert_eq!(child.signal, Some(9));
    }
}
