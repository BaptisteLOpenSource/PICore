use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::git;
use crate::model::ProjectId;
use crate::process::ProcError;

pub fn discover(
    roots: &[PathBuf],
    max_depth: usize,
    ignore_dirs: &[String],
) -> Result<Vec<ProjectId>> {
    let mut found = Vec::new();
    for root in roots {
        walk(root, 0, max_depth, ignore_dirs, &mut found)?;
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.dedup();
    Ok(found)
}

pub fn suggest_roots(home: &Path) -> Vec<PathBuf> {
    let ignore = Config::default().ignore_dirs;
    ["code", "dev", "projects", "src", "Documents"]
        .iter()
        .map(|d| home.join(d))
        .filter(|p| {
            p.is_dir()
                && discover(std::slice::from_ref(p), 3, &ignore).is_ok_and(|ids| !ids.is_empty())
        })
        .collect()
}

fn walk(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    ignore_dirs: &[String],
    out: &mut Vec<ProjectId>,
) -> Result<()> {
    if is_repo_dir(dir) {
        if let Some(id) = project_of(dir)? {
            out.push(id);
        }
        return Ok(());
    }
    if depth == max_depth {
        return Ok(());
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // symlink_metadata: a symlinked dir is not `is_dir`, so loops are never followed.
        if !fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if ignore_dirs.contains(&name) || (name.starts_with('.') && !is_bare_repo_dir(&path)) {
            continue;
        }
        walk(&path, depth + 1, max_depth, ignore_dirs, out)?;
    }
    Ok(())
}

fn is_repo_dir(dir: &Path) -> bool {
    dir.join(".git").exists() || is_bare_repo_dir(dir)
}

fn is_bare_repo_dir(dir: &Path) -> bool {
    ["HEAD", "objects", "refs"]
        .iter()
        .all(|f| dir.join(f).exists())
}

// Submodule git dirs live under `.git/modules/`; they belong to their superproject.
// Only a missing git is fatal: any other failure is a broken repo, skipped.
fn project_of(dir: &Path) -> Result<Option<ProjectId>> {
    let common = match git::common_dir(dir) {
        Ok(c) => c,
        Err(e @ Error::Proc(ProcError::NotFound)) => return Err(e),
        Err(_) => return Ok(None),
    };
    if common.ancestors().any(|a| a.ends_with(".git/modules")) {
        return Ok(None);
    }
    Ok(Some(ProjectId(common)))
}
