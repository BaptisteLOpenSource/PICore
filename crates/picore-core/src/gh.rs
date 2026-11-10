use std::path::Path;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct RepoInfo {
    pub description: String,
    pub url: String,
    pub visibility: String,
    pub topics: Vec<String>,
    pub latest_release: Option<String>,
    pub stars: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRepo {
    description: Option<String>,
    url: String,
    visibility: String,
    repository_topics: Option<Vec<Topic>>,
    latest_release: Option<Release>,
    stargazer_count: u64,
}

#[derive(Deserialize)]
struct Topic {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Release {
    tag_name: String,
}

pub fn parse_repo_info(json: &str) -> Result<RepoInfo, serde_json::Error> {
    let raw: RawRepo = serde_json::from_str(json)?;
    Ok(RepoInfo {
        description: raw.description.unwrap_or_default(),
        url: raw.url,
        visibility: raw.visibility,
        topics: raw
            .repository_topics
            .unwrap_or_default()
            .into_iter()
            .map(|t| t.name)
            .collect(),
        latest_release: raw.latest_release.map(|r| r.tag_name),
        stars: raw.stargazer_count,
    })
}

pub fn repo_view(repo: &str) -> Result<RepoInfo, String> {
    let json = process::run(
        "gh",
        Path::new("."),
        &[
            "repo",
            "view",
            "--json",
            "description,url,visibility,repositoryTopics,latestRelease,stargazerCount",
            "--",
            repo,
        ],
        TIMEOUT,
    )
    .map_err(|e| format!("gh: {e}"))?;
    parse_repo_info(&json).map_err(|e| format!("gh: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repo_view_json() {
        let full = r#"{"description":"d","url":"https://github.com/o/n","visibility":"PUBLIC",
            "repositoryTopics":[{"name":"rust"},{"name":"cli"}],
            "latestRelease":{"tagName":"v1.2.0","name":"x"},"stargazerCount":7}"#;
        let i = parse_repo_info(full).unwrap();
        assert_eq!(i.topics, ["rust", "cli"]);
        assert_eq!(i.latest_release.as_deref(), Some("v1.2.0"));
        assert_eq!((i.description.as_str(), i.stars), ("d", 7));
    }

    #[test]
    fn parses_repo_without_topics_or_release() {
        let bare = r#"{"description":null,"url":"u","visibility":"PRIVATE",
            "repositoryTopics":null,"latestRelease":null,"stargazerCount":0}"#;
        let i = parse_repo_info(bare).unwrap();
        assert!(i.topics.is_empty() && i.latest_release.is_none() && i.description.is_empty());
        assert!(parse_repo_info("nope").is_err());
    }
}
