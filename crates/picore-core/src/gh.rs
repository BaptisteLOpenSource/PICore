use std::path::Path;

use crate::process::{self, ProcError, TIMEOUT};

pub fn current_user() -> Result<String, ProcError> {
    process::run(
        "gh",
        Path::new("."),
        &["api", "user", "--jq", ".login"],
        TIMEOUT,
    )
}

pub fn issue_title(cwd: &Path, issue: u32) -> Result<String, ProcError> {
    let n = issue.to_string();
    process::run(
        "gh",
        cwd,
        &["issue", "view", &n, "--json", "title", "--jq", ".title"],
        TIMEOUT,
    )
    .map(|t| t.trim().to_owned())
}
