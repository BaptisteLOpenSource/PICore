use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::Config;
use crate::model::{Project, project_dir};

/// The one listing shape: CLI `list --json`, MCP `list_projects`, GUI overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectView {
    /// Project path (work tree, else git dir), accepted back everywhere.
    pub id: PathBuf,
    /// `<parent>/<name>` when another Project shares the bare name.
    pub name: String,
    pub product: Option<String>,
    pub branch: Option<String>,
    /// `owner/name` of the origin remote.
    pub repo: Option<String>,
    pub techs: Vec<String>,
    /// All Worktrees, main included.
    pub worktrees: usize,
}

pub fn of(p: &Project, all: &[Project], cfg: &Config) -> ProjectView {
    ProjectView {
        id: p.path().to_path_buf(),
        name: display_name(p, all),
        product: cfg.product_of(project_dir(&p.id)).map(str::to_owned),
        branch: p.branch.clone(),
        repo: p.repo.clone(),
        techs: p.techs.clone(),
        worktrees: p.worktrees.len(),
    }
}

pub fn list(all: &[Project], cfg: &Config) -> Vec<ProjectView> {
    all.iter().map(|p| of(p, all, cfg)).collect()
}

fn display_name(p: &Project, all: &[Project]) -> String {
    if all.iter().filter(|o| o.name == p.name).count() < 2 {
        return p.name.clone();
    }
    match project_dir(&p.id).parent().and_then(Path::file_name) {
        Some(parent) => format!("{}/{}", parent.to_string_lossy(), p.name),
        None => p.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProjectId, project_name};

    fn project(dir: &str) -> Project {
        let id = ProjectId(PathBuf::from(dir).join(".git"));
        Project {
            name: project_name(&id),
            id,
            work_tree: Some(PathBuf::from(dir)),
            branch: Some("main".into()),
            repo: None,
            techs: vec![],
            worktrees: vec![],
            stamp: 0,
        }
    }

    #[test]
    fn id_is_work_tree_and_duplicates_are_qualified() {
        let all = [project("/a/core"), project("/b/core"), project("/c/solo")];
        let names: Vec<_> = list(&all, &Config::default())
            .into_iter()
            .map(|v| (v.id, v.name))
            .collect();
        assert_eq!(
            names,
            [
                (PathBuf::from("/a/core"), "a/core".into()),
                (PathBuf::from("/b/core"), "b/core".into()),
                (PathBuf::from("/c/solo"), "solo".to_owned()),
            ]
        );
    }
}
