use std::path::Path;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::gh;
use crate::git;
use crate::model::Project;
use crate::process::{self, ProcError, TIMEOUT};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Ko,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectHealth {
    pub dirty: u32,
    pub ahead: u32,
    pub behind: u32,
    pub upstream: Option<String>,
    pub remote: Option<Check>,
}

fn check(name: &str, status: Status, message: &str, hint: Option<&str>) -> Check {
    Check {
        name: name.to_owned(),
        status,
        message: message.to_owned(),
        hint: hint.map(str::to_owned),
    }
}

pub fn classify_git(r: std::result::Result<String, ProcError>) -> Check {
    match r {
        Ok(v) => check("git", Status::Ok, v.trim(), None),
        Err(ProcError::Timeout) => check("git", Status::Unknown, "timed out", None),
        Err(ProcError::NotFound) => check("git", Status::Ko, "git not installed", None),
        Err(e) => check("git", Status::Ko, &e.to_string(), None),
    }
}

pub fn classify_ssh_agent(r: std::result::Result<String, ProcError>) -> Check {
    match r {
        Ok(_) => check("ssh-agent", Status::Ok, "agent has keys", None),
        Err(ProcError::Failed { code: Some(1), .. }) => check(
            "ssh-agent",
            Status::Ko,
            "agent has no keys",
            Some("ssh-add"),
        ),
        Err(ProcError::Failed { code: Some(2), .. }) => {
            check("ssh-agent", Status::Ko, "no ssh agent", None)
        }
        Err(_) => check(
            "ssh-agent",
            Status::Unknown,
            "could not check ssh agent",
            None,
        ),
    }
}

pub fn classify_gh(r: std::result::Result<String, ProcError>) -> Check {
    match r {
        Ok(login) => check(
            "gh",
            Status::Ok,
            &format!("logged in as {}", login.trim()),
            None,
        ),
        Err(ProcError::NotFound) => check(
            "gh",
            Status::Ko,
            "gh not installed",
            Some("https://cli.github.com"),
        ),
        Err(ProcError::Failed { code: Some(4), .. }) => {
            check("gh", Status::Ko, "not authenticated", Some("gh auth login"))
        }
        Err(_) => check("gh", Status::Unknown, "could not check gh", None),
    }
}

fn classify_remote(r: Result<()>) -> Check {
    match r {
        Ok(()) => check("remote", Status::Ok, "reachable", None),
        Err(Error::Proc(ProcError::Failed { stderr, .. })) if is_auth_failure(&stderr) => {
            check("remote", Status::Ko, "authentication failed", None)
        }
        Err(_) => check("remote", Status::Unknown, "unreachable or timed out", None),
    }
}

// Git exits 128 for auth and network alike: stderr decides.
fn is_auth_failure(stderr: &str) -> bool {
    [
        "Authentication failed",
        "could not read Username",
        "could not read Password",
        "terminal prompts disabled",
        "Permission denied",
    ]
    .iter()
    .any(|needle| stderr.contains(needle))
}

pub fn machine_health() -> Vec<Check> {
    let cwd = Path::new(".");
    vec![
        classify_git(process::run("git", cwd, &["--version"], TIMEOUT)),
        classify_ssh_agent(process::run("ssh-add", cwd, &["-l"], TIMEOUT)),
        classify_gh(gh::current_user()),
    ]
}

pub fn project_health(p: &Project, remote: bool) -> Result<ProjectHealth> {
    let mut h = ProjectHealth {
        dirty: 0,
        ahead: 0,
        behind: 0,
        upstream: None,
        remote: None,
    };
    if let Some(wt) = &p.work_tree {
        let s = git::status(wt)?;
        h.dirty = s.changes;
        h.ahead = s.ahead;
        h.behind = s.behind;
        h.upstream = s.upstream;
    }
    if remote {
        let dir = p.work_tree.as_deref().unwrap_or(&p.id.0);
        h.remote = Some(classify_remote(git::ls_remote(dir)));
    }
    Ok(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed(code: i32) -> ProcError {
        ProcError::Failed {
            code: Some(code),
            stderr: String::new(),
        }
    }

    #[test]
    fn gh_exit_4_is_ko_with_hint() {
        let c = classify_gh(Err(failed(4)));
        assert_eq!(c.status, Status::Ko);
        assert_eq!(c.hint.as_deref(), Some("gh auth login"));
    }

    #[test]
    fn gh_timeout_is_unknown() {
        assert_eq!(classify_gh(Err(ProcError::Timeout)).status, Status::Unknown);
    }

    #[test]
    fn gh_logged_in_is_ok() {
        let c = classify_gh(Ok("octocat\n".into()));
        assert_eq!(
            (c.status, c.message.as_str()),
            (Status::Ok, "logged in as octocat")
        );
    }

    #[test]
    fn missing_git_is_ko() {
        assert_eq!(classify_git(Err(ProcError::NotFound)).status, Status::Ko);
    }

    #[test]
    fn ssh_codes_map_to_ko() {
        assert_eq!(classify_ssh_agent(Err(failed(1))).status, Status::Ko);
        assert_eq!(classify_ssh_agent(Err(failed(2))).status, Status::Ko);
        assert_eq!(
            classify_ssh_agent(Err(ProcError::NotFound)).status,
            Status::Unknown
        );
    }

    #[test]
    fn remote_auth_failure_is_ko_other_failures_unknown() {
        let auth = ProcError::Failed {
            code: Some(128),
            stderr: "fatal: could not read Username for 'https://h': terminal prompts disabled"
                .into(),
        };
        assert_eq!(classify_remote(Err(Error::Proc(auth))).status, Status::Ko);
        let net = ProcError::Failed {
            code: Some(128),
            stderr: "fatal: unable to access: Could not resolve host".into(),
        };
        assert_eq!(
            classify_remote(Err(Error::Proc(net))).status,
            Status::Unknown
        );
        assert_eq!(
            classify_remote(Err(Error::Proc(ProcError::Timeout))).status,
            Status::Unknown
        );
    }
}
