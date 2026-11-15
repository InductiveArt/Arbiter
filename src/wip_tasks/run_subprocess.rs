use std::fs;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::wip::pull::In;
use crate::wip::scope::Scope;
use crate::wip::task::Task;
use crate::wip::trace::outcome::Failure;
use crate::wip_domain::subprocess::Exit;
use crate::wip_domain::subprocess::Subprocess;

pub fn run_subprocess_task() -> Task {
    Task::new(
        "run_subprocess",
        |scope: &mut Scope, subprocess: In<Subprocess>| run_subprocess(scope, &subprocess),
    )
    .within(Duration::from_secs(30 * 60))
    .line(
        "starts one program in its folder, keeps all it prints and tells how it ended; an \
         exit other than 0 is told, never failed",
    )
}

// How the program ended is told, never judged: only a program that could not
// be run fails the task.
fn run_subprocess(scope: &mut Scope, subprocess: &Subprocess) -> Result<Exit, Failure> {
    scope.note(format!("folder: {}", subprocess.folder.display()));
    if !subprocess.folder.is_dir() {
        return Err(Failure::from(format!(
            "no folder at {}",
            subprocess.folder.display()
        )));
    }
    let mut command = Command::new(&subprocess.program);
    command
        .args(&subprocess.arguments)
        .current_dir(&subprocess.folder);
    let finished = scope.command(&mut command)?;
    let output = scope.dir().join("output");
    fs::create_dir_all(&output).map_err(|error| unwritten(&output, &error))?;
    let exit = Exit {
        code: finished.status.code(),
        signal: finished.status.signal(),
        stdout: output.join("stdout"),
        stderr: output.join("stderr"),
    };
    keep(scope, &exit.stdout, "stdout", &finished.stdout)?;
    keep(scope, &exit.stderr, "stderr", &finished.stderr)?;
    scope.note(format!("ended: {}", exit.ended(&subprocess.program)));
    Ok(exit)
}

// The whole of one stream, never a tail: a cut output hides everything
// printed before the part kept.
fn keep(scope: &mut Scope, file: &Path, stream: &str, printed: &[u8]) -> Result<(), Failure> {
    fs::write(file, printed).map_err(|error| unwritten(file, &error))?;
    scope.note(format!("{stream}: {}", file.display()));
    Ok(())
}

fn unwritten(path: &Path, error: &io::Error) -> Failure {
    Failure::from(format!("{} could not be written: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use super::run_subprocess;
    use crate::wip::job::cancel::Cancel;
    use crate::wip::job::id::JobId;
    use crate::wip::job::id::RunId;
    use crate::wip::layout::Site;
    use crate::wip::scope::Scope;
    use crate::wip::trace::outcome::Outcome;
    use crate::wip_domain::subprocess::Exit;
    use crate::wip_domain::subprocess::Subprocess;

    // A place of its own per test: the files written under `work`, beside the
    // attempt's own folder.
    fn ground(name: &str, written: &[(&str, &str)]) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "arbiter-subprocess-{name}-{}-{}",
            std::process::id(),
            RunId::next()
        ));
        fs::create_dir_all(root.join("work")).expect("the folder is made");
        fs::create_dir_all(root.join("attempt")).expect("the folder is made");
        for (path, content) in written {
            let file = root.join("work").join(path);
            fs::create_dir_all(file.parent().expect("a file sits in a folder"))
                .expect("the folder is made");
            fs::write(file, content).expect("the file is written");
        }
        root
    }

    fn scope(root: &Path, cancel: Cancel) -> Scope {
        let site = Site {
            dir: root.join("attempt"),
            store: root.join("store"),
        };
        Scope::new(JobId::new(RunId::next(), "run_subprocess", 1), cancel, site)
    }

    // A program started in a folder under `work`, by its words.
    fn subprocess(root: &Path, folder: &str, words: &[&str]) -> Subprocess {
        Subprocess {
            name: "probe".to_string(),
            program: words[0].to_string(),
            arguments: words[1..].iter().map(|word| word.to_string()).collect(),
            folder: root.join("work").join(folder),
        }
    }

    #[test]
    fn a_program_is_started_in_its_folder_and_all_it_prints_is_kept_each_stream_in_a_file() {
        let root = ground("printed", &[("a/name", "first\n")]);
        let mut scope = scope(&root, Cancel::new());
        let told = subprocess(&root, "a", &["sh", "-c", "cat name; echo warned >&2"]);
        let exit = run_subprocess(&mut scope, &told).expect("the program ran");
        let output = root.join("attempt").join("output");
        assert_eq!(
            exit,
            Exit {
                code: Some(0),
                signal: None,
                stdout: output.join("stdout"),
                stderr: output.join("stderr"),
            }
        );
        assert_eq!(fs::read_to_string(&exit.stdout).expect("kept"), "first\n");
        assert_eq!(fs::read_to_string(&exit.stderr).expect("kept"), "warned\n");
        let step = scope.finished("run_subprocess", Outcome::Ok);
        assert_eq!(
            step.evidence,
            [
                format!("folder: {}", root.join("work").join("a").display()),
                format!("stdout: {}", output.join("stdout").display()),
                format!("stderr: {}", output.join("stderr").display()),
                "ended: sh exited with 0".to_string(),
            ]
        );
        assert_eq!(
            step.resources.children.len(),
            1,
            "the program's account is kept"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_exit_other_than_0_is_told_and_never_fails_the_task() {
        let root = ground("non-zero", &[("a/x", "")]);
        let mut scope = scope(&root, Cancel::new());
        let exit = run_subprocess(&mut scope, &subprocess(&root, "a", &["sh", "-c", "exit 3"]))
            .expect("the program ran");
        assert_eq!((exit.code, exit.signal), (Some(3), None));
        let step = scope.finished("run_subprocess", Outcome::Ok);
        assert_eq!(
            step.evidence.last().map(String::as_str),
            Some("ended: sh exited with 3")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_program_ended_by_a_signal_is_told_with_the_signal() {
        let root = ground("signal", &[("a/x", "")]);
        let mut scope = scope(&root, Cancel::new());
        let exit = run_subprocess(
            &mut scope,
            &subprocess(&root, "a", &["sh", "-c", "kill -9 $$"]),
        )
        .expect("the program ran");
        assert_eq!((exit.code, exit.signal), (None, Some(9)));
        assert_eq!(exit.ended("sh"), "sh was ended by signal 9");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_program_that_cannot_be_started_fails_naming_the_program() {
        let root = ground("absent", &[("a/x", "")]);
        let failed = run_subprocess(
            &mut scope(&root, Cancel::new()),
            &subprocess(&root, "a", &["arbiter-no-such-program"]),
        )
        .expect_err("nothing ran");
        assert!(
            failed
                .reason
                .starts_with("arbiter-no-such-program could not be started: "),
            "the reason names the program: {}",
            failed.reason
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_that_is_not_there_fails_naming_the_folder() {
        let root = ground("nowhere", &[("a/x", "")]);
        let failed = run_subprocess(
            &mut scope(&root, Cancel::new()),
            &subprocess(&root, "missing", &["cat", "x"]),
        )
        .expect_err("nothing ran");
        assert_eq!(
            failed.reason,
            format!(
                "no folder at {}",
                root.join("work").join("missing").display()
            )
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_job_told_to_stop_kills_the_program_and_fails_as_cancelled() {
        let root = ground("stopped", &[("a/x", "")]);
        let cancel = Cancel::new();
        cancel.cancel();
        let mut scope = scope(&root, cancel);
        let failed = run_subprocess(&mut scope, &subprocess(&root, "a", &["sleep", "5"]))
            .expect_err("the job was told to stop");
        assert_eq!(failed.reason, "cancelled");
        let step = scope.finished("run_subprocess", Outcome::Cancelled);
        assert!(step.resources.children[0].killed, "the program was killed");
        let _ = fs::remove_dir_all(&root);
    }
}
