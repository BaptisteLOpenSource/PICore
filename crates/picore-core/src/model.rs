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

/// Issue number from a `<number>-<slug>` branch.
pub fn issue_of(branch: &str) -> Option<u32> {
    let (n, _) = branch.split_once('-')?;
    n.bytes()
        .all(|b| b.is_ascii_digit())
        .then(|| n.parse().ok())?
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
    pub issue: Option<u32>,
}

// Not Serialize: outputs go through `view::ProjectView`, `stamp` stays internal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub work_tree: Option<PathBuf>,
    pub branch: Option<String>,
    pub repo: Option<String>,
    pub techs: Vec<String>,
    pub worktrees: Vec<Worktree>,
    pub stamp: i64,
}

impl Project {
    /// What users and agents call the Project: its work tree, else its git dir.
    pub fn path(&self) -> &Path {
        self.work_tree.as_deref().unwrap_or(&self.id.0)
    }

    pub fn linked_worktrees(&self) -> impl Iterator<Item = &Worktree> {
        self.worktrees.iter().filter(|w| !w.is_main)
    }
}

#[cfg(test)]
mod tests {
    use super::issue_of;

    #[test]
    fn issue_from_branch() {
        assert_eq!(issue_of("80-find-project"), Some(80));
        assert_eq!(issue_of("feat/80-x"), None);
        assert_eq!(issue_of("main"), None);
        assert_eq!(issue_of("-x"), None);
        assert_eq!(issue_of("80x-y"), None);
        assert_eq!(issue_of("99999999999-x"), None);
    }
}
