use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::domain::subprocess::Subprocess;
use crate::domain::subprocess::SubprocessExit;

pub fn run_subprocess_task() -> Task {
    Task::new(
        "run_subprocess",
        |context: &mut TaskContext, subprocess: In<Subprocess>| {
            run_subprocess(context, &subprocess)
        },
    )
    .within(Duration::from_secs(30 * 60))
    .cached_when(SubprocessExit::exited_with_zero)
    .line(
        "starts one program in its folder, the configs it is handed written in its own folder, \
         keeps all it prints and tells how it ended; an exit other than 0 is told, never \
         failed, and only an exit with 0 is taken from the cache",
    )
}

// The program run with what it is handed and all it prints kept in the
// attempt's own folder.
fn run_subprocess(
    context: &mut TaskContext,
    subprocess: &Subprocess,
) -> Result<SubprocessExit, TaskFailure> {
    let kept = context.dir().to_path_buf();
    run_subprocess_in(context, subprocess, &kept)
}

// How the program ended is told as it ended: only a program that could not
// be run fails the task. Its configs and all it prints are kept in the folder
// given, so nothing changes in the folder the program runs in.
pub fn run_subprocess_in(
    context: &mut TaskContext,
    subprocess: &Subprocess,
    kept: &Path,
) -> Result<SubprocessExit, TaskFailure> {
    context.note(format!("folder: {}", subprocess.folder.display()));
    if !subprocess.folder.is_dir() {
        return Err(TaskFailure::from(format!(
            "no folder at {}",
            subprocess.folder.display()
        )));
    }
    let arguments = handed(context, subprocess, kept)?;
    let mut command = Command::new(&subprocess.program);
    command.args(&arguments).current_dir(&subprocess.folder);
    let finished = context.command(&mut command)?;
    let output = kept.join("output");
    fs::create_dir_all(&output).map_err(|error| unwritten(&output, &error))?;
    keep(context, &output.join("stdout"), "stdout", &finished.stdout)?;
    keep(context, &output.join("stderr"), "stderr", &finished.stderr)?;
    let exit = SubprocessExit {
        code: finished.status.code(),
        signal: finished.status.signal(),
        stdout: String::from_utf8_lossy(&finished.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&finished.stderr).into_owned(),
    };
    context.note(format!("ended: {}", exit.ended(&subprocess.program)));
    Ok(exit)
}

// The configs written in the folder given, and the words with `{pack}` given
// where they were written.
fn handed(
    context: &mut TaskContext,
    subprocess: &Subprocess,
    kept: &Path,
) -> Result<Vec<String>, TaskFailure> {
    if subprocess.configs.is_empty() {
        return Ok(subprocess.arguments.clone());
    }
    let folder = kept.join("config");
    fs::create_dir_all(&folder).map_err(|error| unwritten(&folder, &error))?;
    for config in &subprocess.configs {
        if !plain(&config.name) {
            return Err(TaskFailure::from(format!(
                "the config {} is not a plain file name",
                config.name
            )));
        }
        let file = folder.join(&config.name);
        fs::write(&file, &config.content).map_err(|error| unwritten(&file, &error))?;
    }
    context.note(format!("configs: {}", folder.display()));
    let pointed = folder.to_string_lossy();
    Ok(subprocess
        .arguments
        .iter()
        .map(|word| word.replace("{pack}", &pointed))
        .collect())
}

// A name that stays inside the folder it is joined to.
fn plain(name: &str) -> bool {
    Path::new(name).file_name().and_then(OsStr::to_str) == Some(name)
}

// The whole of one stream, never a tail: a cut output hides everything
// printed before the part kept.
fn keep(
    context: &mut TaskContext,
    file: &Path,
    stream: &str,
    printed: &[u8],
) -> Result<(), TaskFailure> {
    fs::write(file, printed).map_err(|error| unwritten(file, &error))?;
    context.note(format!("{stream}: {}", file.display()));
    Ok(())
}

fn unwritten(path: &Path, error: &io::Error) -> TaskFailure {
    TaskFailure::from(format!("{} could not be written: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use task_runner::context::stop_token::StopToken;
    use task_runner::context::task_context::TaskContext;
    use task_runner::contract::RunId;
    use task_runner::contract::RunRecord;
    use task_runner::contract::TaskAttemptId;

    use super::run_subprocess;
    use crate::domain::ecosystem::Config;
    use crate::domain::subprocess::Subprocess;
    use crate::domain::subprocess::SubprocessExit;

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

    fn context(root: &Path, cancel: StopToken) -> TaskContext {
        TaskContext::new(
            TaskAttemptId::new(RunId::next(), "run_subprocess", 1),
            cancel,
            root.join("attempt"),
        )
    }

    // A program started in a folder under `work`, by its words.
    fn subprocess(root: &Path, folder: &str, words: &[&str]) -> Subprocess {
        Subprocess {
            name: "probe".to_string(),
            program: words[0].to_string(),
            arguments: words[1..].iter().map(|word| word.to_string()).collect(),
            folder: root.join("work").join(folder),
            configs: Vec::new(),
            cone: None,
        }
    }

    #[test]
    fn a_program_is_started_in_its_folder_and_all_it_prints_is_handed_back_and_kept_each_stream_in_a_file(
    ) {
        let root = ground("printed", &[("a/name", "first\n")]);
        let mut context = context(&root, StopToken::new());
        let told = subprocess(&root, "a", &["sh", "-c", "cat name; echo warned >&2"]);
        let exit = run_subprocess(&mut context, &told).expect("the program ran");
        let output = root.join("attempt").join("output");
        assert_eq!(
            exit,
            SubprocessExit {
                code: Some(0),
                signal: None,
                stdout: "first\n".to_string(),
                stderr: "warned\n".to_string(),
            }
        );
        assert_eq!(
            fs::read_to_string(output.join("stdout")).expect("kept"),
            "first\n"
        );
        assert_eq!(
            fs::read_to_string(output.join("stderr")).expect("kept"),
            "warned\n"
        );
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(
            attempt.evidence,
            [
                format!("folder: {}", root.join("work").join("a").display()),
                format!("stdout: {}", output.join("stdout").display()),
                format!("stderr: {}", output.join("stderr").display()),
                "ended: sh exited with 0".to_string(),
            ]
        );
        assert_eq!(
            attempt.resources().processes.len(),
            1,
            "the program's account is kept"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_exit_other_than_0_is_told_and_never_fails_the_task() {
        let root = ground("non-zero", &[("a/x", "")]);
        let mut context = context(&root, StopToken::new());
        let exit = run_subprocess(
            &mut context,
            &subprocess(&root, "a", &["sh", "-c", "exit 3"]),
        )
        .expect("the program ran");
        assert_eq!((exit.code, exit.signal), (Some(3), None));
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(
            attempt.evidence.last().map(String::as_str),
            Some("ended: sh exited with 3")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_configs_handed_are_written_in_the_attempt_s_folder_where_a_word_holding_pack_points() {
        let root = ground("configs", &[("a/x", "")]);
        let mut context = context(&root, StopToken::new());
        let mut told = subprocess(&root, "a", &["cat", "{pack}/ruff.toml"]);
        told.configs = vec![Config {
            name: "ruff.toml".to_string(),
            content: "line-length = 100\n".to_string(),
        }];
        let exit = run_subprocess(&mut context, &told).expect("the program ran");
        assert_eq!(exit.stdout, "line-length = 100\n");
        let written = root.join("attempt").join("config");
        assert!(
            !root.join("work").join("a").join("config").exists(),
            "nothing is written in the folder the program runs in"
        );
        let attempt = context.finished(RunRecord::passed());
        assert!(attempt
            .evidence
            .contains(&format!("configs: {}", written.display())));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_config_named_as_a_path_is_refused_and_nothing_runs() {
        let root = ground("escape", &[("a/x", "")]);
        let mut told = subprocess(&root, "a", &["touch", "ran"]);
        told.configs = vec![Config {
            name: "../escape.toml".to_string(),
            content: "x".to_string(),
        }];
        let failed = run_subprocess(&mut context(&root, StopToken::new()), &told)
            .expect_err("a config is written nowhere but in the attempt's folder");
        assert_eq!(
            failed.reason,
            "the config ../escape.toml is not a plain file name"
        );
        assert!(!root.join("attempt").join("escape.toml").exists());
        assert!(!root.join("work").join("a").join("ran").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_program_ended_by_a_signal_is_told_with_the_signal() {
        let root = ground("signal", &[("a/x", "")]);
        let mut context = context(&root, StopToken::new());
        let exit = run_subprocess(
            &mut context,
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
            &mut context(&root, StopToken::new()),
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
            &mut context(&root, StopToken::new()),
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
    fn an_attempt_told_to_stop_kills_the_program_and_fails_as_cancelled() {
        let root = ground("stopped", &[("a/x", "")]);
        let token = StopToken::new();
        token.stop();
        let mut context = context(&root, token);
        let failed = run_subprocess(&mut context, &subprocess(&root, "a", &["sleep", "5"]))
            .expect_err("the attempt was told to stop");
        assert_eq!(failed.reason, "cancelled");
        let attempt = context.finished(RunRecord::cancelled());
        assert!(
            attempt.resources().processes[0].killed,
            "the program was killed"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
