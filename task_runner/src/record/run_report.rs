use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::contract::RunId;
use crate::contract::RunRecord;
use crate::contract::RunReport;
use crate::contract::TaskAttempt;
use crate::contract::TaskInvocation;
use crate::contract::Usage;

// How a run ended, the kernel's account of every attempt of its own tasks
// summed, where it lives on disk, and each task's invocation in plan order.
impl RunReport {
    pub fn run(&self) -> RunId {
        self.run.unwrap_or_default()
    }

    pub fn outcome(&self) -> &RunRecord {
        self.outcome.as_ref().unwrap_or(RunRecord::unset())
    }

    pub fn usage(&self) -> &Usage {
        self.usage.as_ref().unwrap_or(Usage::unset())
    }

    pub fn dir(&self) -> PathBuf {
        PathBuf::from(&self.dir)
    }

    pub fn invocation(&self, task: &str) -> Option<&TaskInvocation> {
        self.invocations
            .iter()
            .find(|invocation| invocation.task == task)
    }

    pub fn attempts_of(&self, task: &str) -> &[TaskAttempt] {
        self.invocation(task)
            .map_or(&[], |invocation| &invocation.attempts)
    }

    // The last attempt at the task: how it ended.
    pub fn attempt(&self, task: &str) -> Option<&TaskAttempt> {
        self.attempts_of(task).last()
    }

    // The report in protobuf's JSON form, which any language reads.
    pub fn write(&self, file: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        fs::write(file, bytes)
    }

    // A report that does not read as the contract is refused as invalid data.
    pub fn read(file: &Path) -> io::Result<RunReport> {
        serde_json::from_slice(&fs::read(file)?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::process;

    use crate::contract::RunId;
    use crate::contract::RunRecord;
    use crate::contract::RunReport;

    #[test]
    fn a_report_written_reads_back_whole_even_a_float_printed_at_full_length() {
        let run = RunId::next();
        let file = env::temp_dir().join(format!("task-runner-report-{}-{run}.json", process::id()));
        // A float whose shortest text a fast reader turns into its neighbour.
        let report = RunReport {
            run: Some(run),
            outcome: Some(RunRecord::passed()),
            seconds: 1.071_015_582_999_999_9,
            dir: "runs".to_string(),
            ..RunReport::default()
        };
        report.write(&file).expect("the report is written");
        let read = RunReport::read(&file).expect("the report is read");
        let _ = fs::remove_file(&file);
        assert_eq!(read, report);
    }
}
