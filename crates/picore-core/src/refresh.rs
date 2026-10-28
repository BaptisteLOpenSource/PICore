use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::Config;
use crate::discovery::discover;
use crate::error::Result;
use crate::git;
use crate::model::{Project, ProjectId, Worktree, project_name};
use crate::store::Index;
use crate::tech;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Fresh,
    Updated,
    Gone,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct IndexReport {
    pub found: usize,
    pub updated: usize,
    pub removed: usize,
}

enum Step {
    Gone,
    Fresh,
    Scanned(Project),
}

// Root mtime: git deletes an empty `worktrees/` on last `worktree remove`, which only the root sees.
pub fn stamp(id: &ProjectId) -> Result<i64> {
    let mut acc = fs::metadata(&id.0)?.modified()?;
    for rel in ["HEAD", "packed-refs", "refs", "worktrees"] {
        acc = newest(&id.0.join(rel), acc)?;
    }
    Ok(acc
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64)
}

fn newest(path: &Path, acc: SystemTime) -> io::Result<SystemTime> {
    // Absent entries (no packed-refs, no worktrees/) are normal, not errors.
    let Ok(meta) = fs::metadata(path) else {
        return Ok(acc);
    };
    let mut acc = acc.max(meta.modified()?);
    if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            acc = newest(&entry?.path(), acc)?;
        }
    }
    Ok(acc)
}

pub fn scan(id: &ProjectId) -> Result<Project> {
    let stamp = stamp(id)?;
    let mut worktrees = Vec::new();
    // Bare entry is listed first and carries no work tree; the first non-bare entry is main.
    for (i, e) in git::worktrees(&id.0)?
        .into_iter()
        .enumerate()
        .filter(|(_, e)| !e.bare)
    {
        let last_commit_at = if e.path.is_dir() {
            git::last_commit_at(&e.path)?
        } else {
            None
        };
        worktrees.push(Worktree {
            is_main: i == 0,
            // An unborn worktree reports an all-zero HEAD.
            head: e.head.filter(|h| h.bytes().any(|b| b != b'0')),
            branch: e.branch,
            detached: e.detached,
            locked: e.locked,
            last_commit_at,
            path: e.path,
        });
    }
    let main = worktrees.iter().find(|w| w.is_main);
    let work_tree = main.map(|w| w.path.clone());
    let branch = main.and_then(|w| w.branch.clone());
    let techs = work_tree.as_deref().map(tech::detect).unwrap_or_default();
    Ok(Project {
        id: id.clone(),
        name: project_name(id),
        work_tree,
        branch,
        techs,
        worktrees,
        stamp,
    })
}

pub fn refresh(index: &Index, id: &ProjectId) -> Result<Outcome> {
    let known = index.get(id)?.map(|p| p.stamp);
    let step = plan(id, known)?;
    apply(index, id, step)
}

pub fn refresh_many(index: &Index, ids: &[ProjectId]) -> Vec<(ProjectId, Result<Outcome>)> {
    // Index is not Sync: stored stamps are read here, only git work runs in threads.
    let known: Vec<_> = ids
        .iter()
        .map(|id| index.get(id).map(|p| p.map(|p| p.stamp)))
        .collect();
    let steps: Vec<Result<Step>> = thread::scope(|s| {
        let handles: Vec<_> = ids
            .iter()
            .zip(known)
            .map(|(id, known)| s.spawn(move || known.and_then(|k| plan(id, k))))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("refresh thread panicked"))
            .collect()
    });
    ids.iter()
        .zip(steps)
        .map(|(id, step)| (id.clone(), step.and_then(|s| apply(index, id, s))))
        .collect()
}

pub fn index_all(index: &Index, cfg: &Config) -> Result<IndexReport> {
    let ids = discover(&cfg.roots, cfg.max_depth, &cfg.ignore_dirs)?;
    let mut report = IndexReport {
        found: ids.len(),
        ..IndexReport::default()
    };
    for (_, outcome) in refresh_many(index, &ids) {
        match outcome? {
            Outcome::Updated => report.updated += 1,
            Outcome::Gone => report.removed += 1,
            Outcome::Fresh => {}
        }
    }
    report.removed += index.retain(&ids)?;
    Ok(report)
}

fn plan(id: &ProjectId, known: Option<i64>) -> Result<Step> {
    if !id.0.exists() {
        return Ok(Step::Gone);
    }
    if known == Some(stamp(id)?) {
        return Ok(Step::Fresh);
    }
    Ok(Step::Scanned(scan(id)?))
}

fn apply(index: &Index, id: &ProjectId, step: Step) -> Result<Outcome> {
    match step {
        Step::Gone => {
            index.remove(id)?;
            Ok(Outcome::Gone)
        }
        Step::Fresh => Ok(Outcome::Fresh),
        Step::Scanned(p) => {
            index.upsert(&p)?;
            Ok(Outcome::Updated)
        }
    }
}
