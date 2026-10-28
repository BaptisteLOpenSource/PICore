use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::git;
use crate::model::{Project, Worktree};

const DAY: i64 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Verdict {
    Prunable(Reason),
    Kept(Blocker),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Reason {
    Missing,
    Merged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Blocker {
    Main,
    Detached,
    Locked,
    Changes(u32),
    EnvFile,
    NotMerged,
    NoDefaultBranch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assessment {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub verdict: Verdict,
    pub stale: bool,
}

pub fn assess(p: &Project, wt: &Worktree, stale_days: u32, now: i64) -> Result<Assessment> {
    let stale = wt
        .last_commit_at
        .is_some_and(|t| now - t > i64::from(stale_days) * DAY);
    Ok(Assessment {
        path: wt.path.clone(),
        branch: wt.branch.clone(),
        verdict: verdict(&p.id.0, wt)?,
        stale,
    })
}

fn verdict(repo: &Path, wt: &Worktree) -> Result<Verdict> {
    if wt.is_main {
        return Ok(Verdict::Kept(Blocker::Main));
    }
    if wt.locked {
        return Ok(Verdict::Kept(Blocker::Locked));
    }
    if !wt.path.is_dir() {
        return Ok(Verdict::Prunable(Reason::Missing));
    }
    if wt.detached {
        return Ok(Verdict::Kept(Blocker::Detached));
    }
    let changes = git::status(&wt.path)?.changes;
    if changes > 0 {
        return Ok(Verdict::Kept(Blocker::Changes(changes)));
    }
    if !git::ignored_env_files(&wt.path)?.is_empty() {
        return Ok(Verdict::Kept(Blocker::EnvFile));
    }
    let Some(default) = git::default_branch(repo)? else {
        return Ok(Verdict::Kept(Blocker::NoDefaultBranch));
    };
    // Unborn worktree has no tip: nothing to merge, so it stays.
    let merged = match wt.head.as_deref() {
        Some(head) => git::is_strict_ancestor(repo, head, &default)?,
        None => false,
    };
    Ok(if merged {
        Verdict::Prunable(Reason::Merged)
    } else {
        Verdict::Kept(Blocker::NotMerged)
    })
}

pub fn remove(p: &Project, a: &Assessment, force: bool) -> Result<()> {
    let repo = &p.id.0;
    match &a.verdict {
        Verdict::Prunable(Reason::Missing) => git::prune_worktrees(repo),
        Verdict::Prunable(Reason::Merged) => git::remove_worktree(repo, &a.path, false),
        Verdict::Kept(b @ (Blocker::Main | Blocker::Locked)) => {
            Err(Error::Config(format!("refused: {b:?}")))
        }
        Verdict::Kept(_) if force => git::remove_worktree(repo, &a.path, true),
        Verdict::Kept(b) => Err(Error::Config(format!("kept: {b:?}"))),
    }
}
