use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::git;
use crate::model::{Project, Worktree};

const DAY: i64 = 86_400;
// Not Config ignore_dirs: a discovery setting must not widen deletion.
const REBUILDABLE: [&str; 13] = [
    "node_modules",
    "target",
    ".venv",
    ".next",
    "build",
    "dist",
    "out",
    ".gradle",
    "coverage",
    "__pycache__",
    ".turbo",
    ".cache",
    ".pytest_cache",
];

// JSON: `{"verdict":"kept","reason":"changes"}`, documented in README.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", content = "reason", rename_all = "snake_case")]
pub enum Verdict {
    Prunable(Reason),
    Kept(Blocker),
    /// git failed on this Worktree: never removed, reason = the error.
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Missing,
    Merged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Blocker {
    Main,
    Detached,
    Locked,
    Changes,
    EnvFile,
    IgnoredFiles,
    NotMerged,
    Fresh,
    NoDefaultBranch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assessment {
    pub path: PathBuf,
    pub branch: Option<String>,
    #[serde(flatten)]
    pub verdict: Verdict,
    pub stale: bool,
}

pub fn assess(p: &Project, wt: &Worktree, cfg: &Config, now: i64) -> Assessment {
    let stale = wt
        .last_commit_at
        .is_some_and(|t| now - t > i64::from(cfg.stale_days) * DAY);
    Assessment {
        path: wt.path.clone(),
        branch: wt.branch.clone(),
        verdict: verdict(&p.id.0, wt).unwrap_or_else(|e| Verdict::Unknown(e.to_string())),
        stale,
    }
}

/// Every Worktree of `p`, main included, assessed now.
pub fn assess_project(p: &Project, cfg: &Config) -> Vec<Assessment> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    p.worktrees
        .iter()
        .map(|wt| assess(p, wt, cfg, now))
        .collect()
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
    if git::status(&wt.path)?.changes > 0 {
        return Ok(Verdict::Kept(Blocker::Changes));
    }
    let ignored = git::ignored_files(&wt.path)?;
    if ignored.iter().any(|p| git::is_env_file(p)) {
        return Ok(Verdict::Kept(Blocker::EnvFile));
    }
    // Any enclosing dir counts: build output nests (android/app/build).
    // ls-files lists a nested repo as its dir: its commits are not rebuildable.
    let blocking = ignored.iter().any(|p| {
        if p.join(".git").symlink_metadata().is_ok() {
            return true;
        }
        let dirs = p.strip_prefix(&wt.path).ok().and_then(Path::parent);
        !dirs.is_some_and(|d| {
            d.components()
                .any(|c| REBUILDABLE.iter().any(|r| c.as_os_str() == *r))
        })
    });
    if blocking {
        return Ok(Verdict::Kept(Blocker::IgnoredFiles));
    }
    let Some(default) = git::default_branch(repo)? else {
        return Ok(Verdict::Kept(Blocker::NoDefaultBranch));
    };
    // Unborn worktree has no tip: nothing to merge, so it stays.
    let Some(head) = wt.head.as_deref() else {
        return Ok(Verdict::Kept(Blocker::NotMerged));
    };
    // PRs land in dev before a release reaches the default branch.
    let dev = git::remote_branch(repo, "dev")?;
    let mut merged = false;
    for target in [Some(default), dev].into_iter().flatten() {
        merged = merged || git::is_merged(repo, head, &target)?;
    }
    if !merged {
        return Ok(Verdict::Kept(Blocker::NotMerged));
    }
    // Tip older than the Worktree: no own commit, e.g. a new workspace on a stale base.
    let created = git::created_at(&wt.path)?;
    Ok(if wt.last_commit_at.is_none_or(|t| t < created) {
        Verdict::Kept(Blocker::Fresh)
    } else {
        Verdict::Prunable(Reason::Merged)
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
        Verdict::Unknown(why) => Err(Error::Config(format!("refused: unknown: {why}"))),
    }
}
