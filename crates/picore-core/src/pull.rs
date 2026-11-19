use serde::Serialize;

use crate::error::{Error, Result};
use crate::git;
use crate::model::Project;

#[derive(Debug, Serialize)]
pub struct PullReport {
    pub branch: String,
    pub output: String,
}

/// Fast-forward the checked-out branch of the Project's main work tree.
/// Refuses on local changes, detached HEAD or no upstream.
pub fn pull(p: &Project) -> Result<PullReport> {
    let Some(dir) = p.work_tree.as_deref() else {
        return Err(Error::Refused(format!("{}: bare project", p.name)));
    };
    let s = git::status(dir)?;
    let Some(branch) = s.branch.clone() else {
        return Err(Error::Refused(format!("{}: detached HEAD", p.name)));
    };
    if s.upstream.is_none() {
        return Err(Error::Refused(format!(
            "{}: {branch} has no upstream",
            p.name
        )));
    }
    if s.changes() > 0 {
        return Err(Error::Refused(format!(
            "{}: {} local changes, commit or stash first",
            p.name, s.changes()
        )));
    }
    let output = git::pull_ff_only(dir)?;
    Ok(PullReport { branch, output })
}
