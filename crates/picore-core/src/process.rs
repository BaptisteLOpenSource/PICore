use std::ffi::OsStr;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;
use wait_timeout::ChildExt;

pub const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Error)]
pub enum ProcError {
    #[error("binary not found")]
    NotFound,
    #[error("timed out")]
    Timeout,
    #[error("exited with {code:?}: {stderr}")]
    Failed { code: Option<i32>, stderr: String },
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub fn run(
    program: &str,
    cwd: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<String, ProcError> {
    run_env(program, cwd, args, &[], timeout)
}

pub fn run_env(
    program: &str,
    cwd: &Path,
    args: &[&str],
    env: &[(&str, &OsStr)],
    timeout: Duration,
) -> Result<String, ProcError> {
    let start = Instant::now();
    let mut cmd = Command::new(program);
    // Own process group: a timeout must also kill grandchildren (git -> ssh).
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let mut child = cmd
        .args(args)
        .current_dir(cwd)
        .envs(env.iter().copied())
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => ProcError::NotFound,
            _ => ProcError::Io(e),
        })?;

    // Drain pipes concurrently: a full pipe buffer would block the child before it exits.
    let stdout_reader = drain(child.stdout.take().expect("stdout is piped"));
    let stderr_reader = drain(child.stderr.take().expect("stderr is piped"));

    let Some(status) = child.wait_timeout(timeout)? else {
        kill_group(child.id());
        child.kill()?;
        child.wait()?;
        return Err(ProcError::Timeout);
    };

    // A grandchild can hold the pipes open after the child exits: bound the reads by the deadline.
    let deadline = start + timeout;
    let collected = collect(&stdout_reader, deadline)
        .and_then(|out| collect(&stderr_reader, deadline).map(|err| (out, err)));
    if collected.is_err() {
        kill_group(child.id());
    }
    let (stdout, stderr) = collected?;
    if status.success() {
        Ok(String::from_utf8_lossy(&stdout).into_owned())
    } else {
        Err(ProcError::Failed {
            code: status.code(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        })
    }
}

#[cfg(unix)]
fn kill_group(pid: u32) {
    // SAFETY: plain syscall; pgid = child pid via process_group(0).
    unsafe { libc::kill(-(pid as libc::pid_t), libc::SIGKILL) };
}

#[cfg(not(unix))]
fn kill_group(_pid: u32) {}

fn drain<R: Read + Send + 'static>(mut pipe: R) -> Receiver<io::Result<Vec<u8>>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = tx.send(pipe.read_to_end(&mut buf).map(|_| buf));
    });
    rx
}

fn collect(rx: &Receiver<io::Result<Vec<u8>>>, deadline: Instant) -> Result<Vec<u8>, ProcError> {
    match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(out) => Ok(out?),
        Err(RecvTimeoutError::Timeout) => Err(ProcError::Timeout),
        Err(RecvTimeoutError::Disconnected) => Err(io::Error::other("pipe reader panicked").into()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn run_sets_c_locale() {
        let out = run(
            "sh",
            Path::new("."),
            &["-c", "echo $LC_ALL $GIT_TERMINAL_PROMPT"],
            TIMEOUT,
        )
        .unwrap();
        assert_eq!(out.trim(), "C 0");
    }

    #[test]
    fn run_times_out() {
        let t = Instant::now();
        assert!(matches!(
            run("sleep", Path::new("."), &["10"], Duration::from_millis(200)),
            Err(ProcError::Timeout)
        ));
        assert!(t.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn run_bounds_pipe_reads_when_grandchild_holds_pipes() {
        let t = Instant::now();
        assert!(matches!(
            run(
                "sh",
                Path::new("."),
                &["-c", "(sleep 10 &); echo hi"],
                Duration::from_secs(1),
            ),
            Err(ProcError::Timeout)
        ));
        assert!(t.elapsed() < Duration::from_secs(3));
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_grandchildren() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("pid");
        let script = format!("sleep 30 & echo $! > {}; wait", pid_file.display());
        assert!(matches!(
            run(
                "sh",
                dir.path(),
                &["-c", &script],
                Duration::from_millis(300)
            ),
            Err(ProcError::Timeout)
        ));
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let alive = || run("kill", dir.path(), &["-0", pid.trim()], TIMEOUT).is_ok();
        let t = Instant::now();
        while alive() && t.elapsed() < Duration::from_secs(2) {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(), "grandchild {pid} survived");
    }

    #[test]
    fn run_missing_binary() {
        assert!(matches!(
            run("picore-no-such-bin", Path::new("."), &[], TIMEOUT),
            Err(ProcError::NotFound)
        ));
    }

    #[test]
    fn run_nonzero_exit() {
        assert!(matches!(
            run("sh", Path::new("."), &["-c", "echo boom >&2; exit 3"], TIMEOUT),
            Err(ProcError::Failed { code: Some(3), ref stderr }) if stderr.contains("boom")
        ));
    }
}
