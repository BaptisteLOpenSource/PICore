use std::path::PathBuf;

use serde::Serialize;

use crate::error::Result;
use crate::gh::{self, RepoInfo};
use crate::git;
use crate::model::project_dir;
use crate::resolve::Target;

const TAGS: usize = 5;

#[derive(Debug, Serialize)]
pub struct Info {
    pub name: String,
    pub product: Option<String>,
    pub path: PathBuf,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub tags: Vec<String>,
    pub github: Option<RepoInfo>,
    pub github_error: Option<String>,
}

pub fn info(t: &Target, product: Option<&str>) -> Result<Info> {
    let p = t.project;
    let path = t
        .worktree
        .map(|w| w.path.clone())
        .or_else(|| p.work_tree.clone())
        .unwrap_or_else(|| project_dir(&p.id).to_owned());
    let branch = t.worktree.map_or(&p.branch, |w| &w.branch).clone();
    let (github, github_error) = match p.repo.as_deref().map(gh::repo_view) {
        Some(Ok(g)) => (Some(g), None),
        Some(Err(e)) => (None, Some(e)),
        None => (
            None,
            Some("no GitHub repo: origin remote not set".to_owned()),
        ),
    };
    Ok(Info {
        name: p.name.clone(),
        product: product.map(str::to_owned),
        tags: git::tags(&path, TAGS)?,
        path,
        repo: p.repo.clone(),
        branch,
        github,
        github_error,
    })
}
