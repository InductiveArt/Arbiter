use std::fs;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use crate::kernel::child::exited;
use crate::kernel::child::kill;
use crate::kernel::child::own_group;
use crate::kernel::child::reap;
use crate::kernel::child::terminate;
use crate::layout::STARTED_FILE;
use crate::layout::STDERR_FILE;
use crate::layout::STDOUT_FILE;
use crate::run::medium::Ended;
use crate::run::medium::Medium;
use crate::run::refusal::Problem;

// Attempts as processes of the worker binary: each is told its folder, leads
// a process group of its own when asked, has its output pumped to disk up to
// the cap, and its pumps are given this long to drain once it ended.
pub struct Processes {
    worker: PathBuf,
    grouped: bool,
    cap: u64,
    drain: Duration,
}

impl Processes {
    pub fn new(worker: PathBuf, grouped: bool, cap: u64, drain: Duration) -> Processes {
        Processes {
            worker,
            grouped,
            cap,
            drain,
        }
    }
}

// One attempt out as a process: the child, its folder, and the pumps carrying
// its output to disk.
pub struct Running {
    child: Child,
    dir: PathBuf,
    stdout: Pump,
    stderr: Pump,
}

impl Medium for Processes {
    type Handle = Running;

    fn verify(&self) -> Vec<Problem> {
        let reason = match fs::metadata(&self.worker) {
            Ok(meta) if !meta.is_file() => Some("it is not a file".to_string()),
            Ok(meta) if meta.permissions().mode() & 0o111 == 0 => {
                Some("it is not executable".to_string())
            }
            Ok(_) => None,
            Err(error) => Some(error.to_string()),
        };
        reason
            .map(|reason| Problem::NoWorker {
                path: self.worker.clone(),
                reason,
            })
            .into_iter()
            .collect()
    }

    fn start(&mut self, dir: &Path) -> Result<Running, String> {
        let mut command = Command::new(&self.worker);
        command
            .arg("worker")
            .arg(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if self.grouped {
            own_group(&mut command);
        }
        let mut child = command.spawn().map_err(|error| error.to_string())?;
        let stdout = pump(child.stdout.take(), dir.join(STDOUT_FILE), self.cap);
        let stderr = pump(child.stderr.take(), dir.join(STDERR_FILE), self.cap);
        Ok(Running {
            child,
            dir: dir.to_path_buf(),
            stdout,
            stderr,
        })
    }

    fn started(&self, running: &Running) -> bool {
        running.dir.join(STARTED_FILE).exists()
    }

    fn stop(&mut self, running: &Running) {
        let _ = terminate(&running.child, self.grouped);
    }

    fn force(&mut self, running: &Running) {
        let _ = kill(&running.child, self.grouped);
    }

    // A group leader that ended is seen before it is reaped, while its pid is
    // still held, so what it left behind in its group is ended with it.
    fn poll(&mut self, running: &mut Running) -> Option<Ended> {
        let result = match exited(&running.child) {
            Ok(true) => {
                if self.grouped {
                    let _ = kill(&running.child, true);
                }
                reap(&mut running.child, true)
            }
            Ok(false) => reap(&mut running.child, false),
            Err(error) => Err(error),
        };
        let (reaped, fault) = match result {
            Ok(None) => return None,
            Ok(Some(reaped)) => (Some(reaped), None),
            Err(error) => (
                None,
                Some(format!("the worker could not be reaped: {error}")),
            ),
        };
        let stdout = running
            .stdout
            .finish(self.drain)
            .unwrap_or_else(|| sized(&running.dir.join(STDOUT_FILE)));
        let stderr = running
            .stderr
            .finish(self.drain)
            .unwrap_or_else(|| sized(&running.dir.join(STDERR_FILE)));
        Some(Ended {
            program: self.worker.display().to_string(),
            args: vec!["worker".to_string(), running.dir.display().to_string()],
            exit_code: reaped.as_ref().and_then(|reaped| reaped.exit_code),
            signal: reaped.as_ref().and_then(|reaped| reaped.signal),
            usage: reaped.map(|reaped| reaped.usage).unwrap_or_default(),
            stdout,
            stderr,
            fault,
        })
    }
}

// Carries one pipe to a file as the process writes it: everything is read so
// the process never stalls, up to the cap is kept, and both counts come back
// once the pipe closes.
struct Pump {
    counts: Receiver<(u64, u64)>,
}

impl Pump {
    fn finish(&self, within: Duration) -> Option<(u64, u64)> {
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

// What a pump could not report is read from the file it filled.
fn sized(file: &Path) -> (u64, u64) {
    let bytes = fs::metadata(file).map(|meta| meta.len()).unwrap_or(0);
    (bytes, bytes)
}
