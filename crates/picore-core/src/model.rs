use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub PathBuf);

pub fn project_dir(id: &ProjectId) -> &Path {
    match id.0.file_name() {
        Some(n) if n == ".git" || n == ".bare" => id.0.parent().unwrap_or(&id.0),
        _ => &id.0,
    }
}

pub fn project_name(id: &ProjectId) -> String {
    let name = project_dir(id)
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    name.strip_suffix(".git").unwrap_or(&name).to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub is_main: bool,
    pub detached: bool,
    pub locked: bool,
    pub last_commit_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub work_tree: Option<PathBuf>,
    pub branch: Option<String>,
    pub techs: Vec<String>,
    pub worktrees: Vec<Worktree>,
    pub stamp: i64,
}

impl Project {
    pub fn linked_worktrees(&self) -> impl Iterator<Item = &Worktree> {
        self.worktrees.iter().filter(|w| !w.is_main)
    }
}
