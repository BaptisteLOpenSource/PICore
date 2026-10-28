use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
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
        child.kill()?;
        child.wait()?;
        return Err(ProcError::Timeout);
    };

    let stdout = join(stdout_reader)?;
    let stderr = join(stderr_reader)?;
    if status.success() {
        Ok(String::from_utf8_lossy(&stdout).into_owned())
    } else {
        Err(ProcError::Failed {
            code: status.code(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        })
    }
}

fn drain<R: Read + Send + 'static>(mut pipe: R) -> JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        pipe.read_to_end(&mut buf).map(|_| buf)
    })
}

fn join(handle: JoinHandle<io::Result<Vec<u8>>>) -> io::Result<Vec<u8>> {
    handle
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("pipe reader panicked")))
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
