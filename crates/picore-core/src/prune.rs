use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::git;
use crate::model::{Project, Worktree, project_dir};
use crate::wtname;

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

/// Whether the Worktree's work has landed, apart from what blocks removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeState {
    /// In `origin/<default>` or `origin/dev` (merged or squash-merged).
    Merged,
    /// Only in local `<default>` or `dev`: not pushed yet.
    MergedLocal,
    /// Not landed; merges cleanly into dev (else default).
    Unmerged,
    /// Not landed; would conflict with dev (else default).
    Conflicts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assessment {
    pub path: PathBuf,
    pub branch: Option<String>,
    #[serde(flatten)]
    pub verdict: Verdict,
    /// None: main, missing dir, unborn branch, no default branch, or git failed.
    pub merge: Option<MergeState>,
    pub stale: bool,
}

pub fn assess(p: &Project, wt: &Worktree, cfg: &Config, now: i64) -> Assessment {
    let stale = wt
        .last_commit_at
        .is_some_and(|t| now - t > i64::from(cfg.stale_days) * DAY);
    let merge = match wt.head.as_deref() {
        Some(head) if !wt.is_main && wt.path.is_dir() => merge_state(&p.id.0, head),
        _ => Ok(None),
    };
    Assessment {
        path: wt.path.clone(),
        branch: wt.branch.clone(),
        verdict: verdict(wt, &merge).unwrap_or_else(|e| Verdict::Unknown(e.to_string())),
        merge: merge.ok().flatten().map(|(m, _)| m),
        stale,
    }
}

// bool = removable: a remote target holds the tip and it moved past it.
fn merge_state(repo: &Path, head: &str) -> Result<Option<(MergeState, bool)>> {
    use git::MergeTree::{Conflicts, Merged, SameTip};
    let Some(default) = git::default_branch(repo)? else {
        return Ok(None);
    };
    // dev first: PRs land there, so it judges conflict risk.
    let remote = git::remote_branch(repo, "dev")?
        .into_iter()
        .chain([default.clone()]);
    let (mut risk, mut same_tip) = (None, false);
    for target in remote {
        match git::merge_tree(repo, head, &target)? {
            Merged => return Ok(Some((MergeState::Merged, true))),
            SameTip => same_tip = true,
            m => _ = risk.get_or_insert(m),
        }
    }
    if same_tip {
        return Ok(Some((MergeState::Merged, false)));
    }
    let local_default = default.strip_prefix("origin/").unwrap_or(&default);
    for name in ["dev", local_default] {
        if let Some(local) = git::local_branch(repo, name)?
            && matches!(git::merge_tree(repo, head, &local)?, SameTip | Merged)
        {
            return Ok(Some((MergeState::MergedLocal, false)));
        }
    }
    let state = if risk == Some(Conflicts) {
        MergeState::Conflicts
    } else {
        MergeState::Unmerged
    };
    Ok(Some((state, false)))
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

fn verdict(wt: &Worktree, merge: &Result<Option<(MergeState, bool)>>) -> Result<Verdict> {
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
    // Unborn worktree has no tip: nothing to merge, so it stays.
    if wt.head.is_none() {
        return Ok(Verdict::Kept(Blocker::NotMerged));
    }
    let merge = match merge {
        Ok(m) => *m,
        Err(e) => return Ok(Verdict::Unknown(e.to_string())),
    };
    let Some((_, removable)) = merge else {
        return Ok(Verdict::Kept(Blocker::NoDefaultBranch));
    };
    // MergedLocal stays: unpushed work is lost with the Worktree.
    if !removable {
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

/// `<Project>.worktrees` when it is a real dir with no entry left.
pub fn empty_container(p: &Project) -> Option<PathBuf> {
    let dir = wtname::container(project_dir(&p.id));
    let real = dir.symlink_metadata().is_ok_and(|m| m.is_dir());
    (real && fs::read_dir(&dir).is_ok_and(|mut e| e.next().is_none())).then_some(dir)
}

/// remove_dir, not remove_dir_all: a file landing since the check stops it.
pub fn remove_container(dir: &Path) -> Result<()> {
    Ok(fs::remove_dir(dir)?)
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
