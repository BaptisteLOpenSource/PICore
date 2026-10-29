use std::env;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::process::{self, ProcError, TIMEOUT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
    pub locked: bool,
    pub prunable: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusSummary {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub changes: u32,
}

// Repo-local config is untrusted: fsmonitor would run a repo-chosen program on status.
fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    git_with(cwd, &[], args)
}

fn git_with(cwd: &Path, config: &[&str], args: &[&str]) -> Result<String> {
    let mut full = vec!["-c", "core.fsmonitor=false"];
    for c in config {
        full.extend(["-c", c]);
    }
    full.extend(args);
    Ok(process::run("git", cwd, &full, TIMEOUT)?)
}

pub fn parse_worktree_list(z: &str) -> Vec<WorktreeEntry> {
    let mut out = Vec::new();
    let mut cur: Option<WorktreeEntry> = None;
    for field in z.split('\0') {
        if let Some(path) = field.strip_prefix("worktree ") {
            out.extend(cur.replace(WorktreeEntry {
                path: PathBuf::from(path),
                head: None,
                branch: None,
                bare: false,
                detached: false,
                locked: false,
                prunable: false,
            }));
        } else if let Some(w) = cur.as_mut() {
            let (key, val) = field.split_once(' ').unwrap_or((field, ""));
            match key {
                "HEAD" => w.head = Some(val.to_owned()),
                "branch" => {
                    w.branch = Some(val.strip_prefix("refs/heads/").unwrap_or(val).to_owned())
                }
                "bare" => w.bare = true,
                "detached" => w.detached = true,
                "locked" => w.locked = true,
                "prunable" => w.prunable = true,
                _ => {}
            }
        }
    }
    out.extend(cur);
    out
}

pub fn parse_status(z: &str) -> StatusSummary {
    let mut s = StatusSummary::default();
    let mut fields = z.split('\0');
    while let Some(field) = fields.next() {
        if let Some(v) = field.strip_prefix("# branch.head ") {
            s.branch = (v != "(detached)").then(|| v.to_owned());
        } else if let Some(v) = field.strip_prefix("# branch.upstream ") {
            s.upstream = Some(v.to_owned());
        } else if let Some(v) = field.strip_prefix("# branch.ab ") {
            let mut counts = v.split(' ');
            s.ahead = counts
                .next()
                .and_then(|a| a.strip_prefix('+')?.parse().ok())
                .unwrap_or(0);
            s.behind = counts
                .next()
                .and_then(|b| b.strip_prefix('-')?.parse().ok())
                .unwrap_or(0);
        } else {
            match field.split_once(' ') {
                Some(("1" | "u" | "?", _)) => s.changes += 1,
                // Rename/copy entries carry the original path as the next NUL field.
                Some(("2", _)) => {
                    s.changes += 1;
                    fields.next();
                }
                _ => {}
            }
        }
    }
    s
}

pub fn worktrees(repo: &Path) -> Result<Vec<WorktreeEntry>> {
    Ok(parse_worktree_list(&git(
        repo,
        &["worktree", "list", "--porcelain", "-z"],
    )?))
}

pub fn status(work_tree: &Path) -> Result<StatusSummary> {
    let filters = filter_blanks(work_tree)?;
    let config: Vec<&str> = filters.iter().map(String::as_str).collect();
    Ok(parse_status(&git_with(
        work_tree,
        &config,
        &["status", "--porcelain=v2", "--branch", "-z"],
    )?))
}

// Re-hashing a stale index entry runs repo-chosen filter drivers: blank them.
fn filter_blanks(work_tree: &Path) -> Result<Vec<String>> {
    let keys = match git(
        work_tree,
        &[
            "config",
            "--local",
            "--null",
            "--name-only",
            "--get-regexp",
            r"^filter\.",
        ],
    ) {
        Ok(out) => out,
        Err(Error::Proc(ProcError::Failed { .. })) => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(keys
        .split('\0')
        .filter(|k| !k.is_empty())
        .map(|k| format!("{k}="))
        .collect())
}

pub fn common_dir(path: &Path) -> Result<PathBuf> {
    let out = git(path, &["rev-parse", "--git-common-dir"])?;
    Ok(path.join(out.trim_end_matches('\n')).canonicalize()?)
}

pub fn last_commit_at(work_tree: &Path) -> Result<Option<i64>> {
    if !verifies(work_tree, "HEAD")? {
        return Ok(None);
    }
    let out = git(work_tree, &["log", "-1", "--format=%ct"])?;
    out.trim()
        .parse()
        .map(Some)
        .map_err(|e| Error::Config(format!("commit time {out:?}: {e}")))
}

pub fn ignored_files(work_tree: &Path) -> Result<Vec<PathBuf>> {
    // ls-files lists each file: status --ignored collapses dirs, hiding .env.
    let out = git(
        work_tree,
        &[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
        ],
    )?;
    Ok(out
        .split('\0')
        .filter(|f| !f.is_empty())
        .map(|p| work_tree.join(p))
        .collect())
}

pub fn is_env_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with(".env"))
}

pub fn default_branch(repo: &Path) -> Result<Option<String>> {
    if let Ok(origin) = git(
        repo,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        let origin = origin.trim_end_matches('\n');
        let name = origin.strip_prefix("origin/").unwrap_or(origin);
        if verifies(repo, &format!("refs/heads/{name}"))? {
            return Ok(Some(name.to_owned()));
        }
        if verifies(repo, &format!("refs/remotes/origin/{name}"))? {
            return Ok(Some(format!("origin/{name}")));
        }
    }
    for name in ["main", "master"] {
        if verifies(repo, &format!("refs/heads/{name}"))? {
            return Ok(Some(name.to_owned()));
        }
    }
    Ok(None)
}

pub fn is_strict_ancestor(repo: &Path, tip: &str, of: &str) -> Result<bool> {
    match git(
        repo,
        &["merge-base", "--is-ancestor", "--end-of-options", tip, of],
    ) {
        Ok(_) => {}
        Err(Error::Proc(ProcError::Failed { code: Some(1), .. })) => return Ok(false),
        Err(e) => return Err(e),
    }
    let rev = |r| git(repo, &["rev-parse", "--end-of-options", r]);
    Ok(rev(tip)? != rev(of)?)
}

pub fn ls_remote(work_tree: &Path) -> Result<()> {
    // ssh prompts on /dev/tty, which GIT_TERMINAL_PROMPT does not cover. The -c also
    // overrides a repo-local core.sshCommand; GIT_SSH_COMMAND from the user wins as is.
    let ssh = if env::var_os("GIT_SSH_COMMAND").is_some() {
        None
    } else {
        let base = git(
            work_tree,
            &["config", "--global", "--get", "core.sshCommand"],
        )
        .map(|v| v.trim().to_owned())
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "ssh".to_owned());
        Some(format!("core.sshCommand={base} -o BatchMode=yes"))
    };
    // Repo-local config can name programs (uploadpack, credential.helper, askPass...):
    // read only the URL, run ls-remote outside the repo, allow https and ssh only.
    let url = git(work_tree, &["config", "--get", "remote.origin.url"])?;
    let mut config = vec![
        "protocol.allow=never",
        "protocol.https.allow=always",
        "protocol.ssh.allow=always",
    ];
    config.extend(ssh.as_deref());
    let args = ["ls-remote", "--heads", "--", url.trim_end_matches('\n')];
    git_with(Path::new("/"), &config, &args).map(|_| ())
}

pub fn remove_worktree(repo: &Path, path: &Path, force: bool) -> Result<()> {
    let path = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "non-UTF-8 worktree path"))?;
    let args = if force {
        vec!["worktree", "remove", "--force", path]
    } else {
        vec!["worktree", "remove", path]
    };
    git(repo, &args).map(|_| ())
}

pub fn prune_worktrees(repo: &Path) -> Result<()> {
    git(repo, &["worktree", "prune"]).map(|_| ())
}

fn verifies(repo: &Path, rev: &str) -> Result<bool> {
    match git(
        repo,
        &["rev-parse", "--verify", "--quiet", "--end-of-options", rev],
    ) {
        Ok(_) => Ok(true),
        Err(Error::Proc(ProcError::Failed { .. })) => Ok(false),
        Err(e) => Err(e),
    }
}
