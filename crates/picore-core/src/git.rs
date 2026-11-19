use std::env;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::process::{self, ProcError, TIMEOUT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
    pub locked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangedFile {
    pub path: String,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusSummary {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub files: Vec<ChangedFile>,
}

impl StatusSummary {
    pub fn changes(&self) -> u32 {
        u32::try_from(self.files.len()).unwrap_or(u32::MAX)
    }
}

// Repo-local config is untrusted: fsmonitor would run a repo-chosen program on status.
fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    git_with(cwd, &[], args)
}

fn git_with(cwd: &Path, config: &[&str], args: &[&str]) -> Result<String> {
    git_env(cwd, config, &[], args)
}

type Env = Vec<(String, OsString)>;

fn git_env(
    cwd: &Path,
    config: &[&str],
    env: &[(String, OsString)],
    args: &[&str],
) -> Result<String> {
    // No optional locks: status would rewrite a stale index inside the Project.
    let mut full = vec!["--no-optional-locks", "-c", "core.fsmonitor=false"];
    for c in config {
        full.extend(["-c", c]);
    }
    full.extend(args);
    let env: Vec<(&str, &OsStr)> = env
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_os_str()))
        .collect();
    Ok(process::run_env("git", cwd, &full, &env, TIMEOUT)?)
}

pub fn parse_worktree_list(z: &str) -> Vec<WorktreeEntry> {
    let mut out = Vec::new();
    let mut cur: Option<WorktreeEntry> = None;
    for field in z.split('\0') {
        if let Some(path) = field.strip_prefix("worktree ") {
            out.extend(cur.replace(WorktreeEntry {
                path: PathBuf::from(path),
                head: None,
                branch: None,
                bare: false,
                detached: false,
                locked: false,
            }));
        } else if let Some(w) = cur.as_mut() {
            let (key, val) = field.split_once(' ').unwrap_or((field, ""));
            match key {
                "HEAD" => w.head = Some(val.to_owned()),
                "branch" => {
                    w.branch = Some(val.strip_prefix("refs/heads/").unwrap_or(val).to_owned())
                }
                "bare" => w.bare = true,
                "detached" => w.detached = true,
                "locked" => w.locked = true,
                _ => {}
            }
        }
    }
    out.extend(cur);
    out
}

pub fn parse_status(z: &str) -> StatusSummary {
    let mut s = StatusSummary::default();
    let mut fields = z.split('\0');
    while let Some(field) = fields.next() {
        if let Some(v) = field.strip_prefix("# branch.head ") {
            s.branch = (v != "(detached)").then(|| v.to_owned());
        } else if let Some(v) = field.strip_prefix("# branch.upstream ") {
            s.upstream = Some(v.to_owned());
        } else if let Some(v) = field.strip_prefix("# branch.ab ") {
            let mut counts = v.split(' ');
            s.ahead = counts
                .next()
                .and_then(|a| a.strip_prefix('+')?.parse().ok());
            s.behind = counts
                .next()
                .and_then(|b| b.strip_prefix('-')?.parse().ok());
        } else if let Some((tag, body)) = field.split_once(' ') {
            // Rename/copy entries carry the original path as the next NUL field.
            let renamed = tag == "2";
            if let Some(file) = change_entry(tag, body) {
                s.files.push(file);
            }
            if renamed {
                fields.next();
            }
        }
    }
    s
}

// Porcelain v2 change lines: fields before the path are fixed per tag.
fn change_entry(tag: &str, body: &str) -> Option<ChangedFile> {
    let before_path = match tag {
        "1" => 7,
        "2" => 8,
        "u" => 9,
        "?" => 0,
        _ => return None,
    };
    let parts: Vec<&str> = body.splitn(before_path + 1, ' ').collect();
    let path = parts.last()?.to_owned();
    let xy = if before_path == 0 { "" } else { parts[0] };
    let kind = match tag {
        "?" => ChangeKind::Untracked,
        "u" => ChangeKind::Conflicted,
        "2" => ChangeKind::Renamed,
        _ if xy.contains('D') => ChangeKind::Deleted,
        _ if xy.starts_with('A') => ChangeKind::Added,
        _ => ChangeKind::Modified,
    };
    Some(ChangedFile {
        path: path.to_owned(),
        kind,
    })
}

pub fn worktrees(repo: &Path) -> Result<Vec<WorktreeEntry>> {
    Ok(parse_worktree_list(&git(
        repo,
        &["worktree", "list", "--porcelain", "-z"],
    )?))
}

// Value of `key` from global or system scope only: a repo-local value must not win.
fn user_config(work_tree: &Path, key: &str) -> String {
    ["--global", "--system"]
        .iter()
        .find_map(|scope| {
            git(work_tree, &["config", scope, "--get", key])
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        })
        .unwrap_or_default()
}

// Network: outlives the 5s status budget. Fast-forward only, never merges.
// Repo-local config names programs (ssh, credential, askPass, proxy, hooks, filters):
// -c beats it, drivers are blanked, https and ssh only, no submodules.
pub fn pull_ff_only(work_tree: &Path) -> Result<String> {
    let blanks = driver_blanks(work_tree)?;
    // Env, not -c: repo-local protocol.<name>.allow beats -c protocol.allow.
    let env: Vec<(&str, &OsStr)> = blanks
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_os_str()))
        .chain([("GIT_ALLOW_PROTOCOL", OsStr::new("https:ssh"))])
        .collect();
    // Signature checks stay as the user set them; their programs come from user scope only.
    let mut config: Vec<String> = [
        "core.fsmonitor=false",
        "core.hooksPath=/dev/null",
        "core.askPass=",
        "core.gitProxy=",
        "credential.helper=",
        "submodule.recurse=false",
        "fetch.recurseSubmodules=false",
        "core.alternateRefsCommand=",
        "pull.rebase=false",
    ]
    .map(String::from)
    .into();
    for key in ["gpg.program", "gpg.ssh.program", "gpg.x509.program"] {
        config.push(format!("{key}={}", user_config(work_tree, key)));
    }
    config.extend(ssh_override(work_tree));
    let args: Vec<&str> = config
        .iter()
        .flat_map(|c| ["-c", c.as_str()])
        .chain(["pull", "--ff-only"])
        .collect();
    Ok(process::run_env(
        "git",
        work_tree,
        &args,
        &env,
        Duration::from_secs(120),
    )?)
}

pub fn status(work_tree: &Path) -> Result<StatusSummary> {
    Ok(parse_status(&git_env(
        work_tree,
        &[],
        &driver_blanks(work_tree)?,
        // driver_blanks cannot reach submodule configs: do not enter them.
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=all",
        ],
    )?))
}

// Re-hashing a stale index entry runs repo-chosen filter drivers: blank repo ones.
// merge-tree runs merge drivers: a user one could fake a merged tree, so set every
// one to `false`, read as a conflict. Env, not -c: a '=' in the name breaks -c.
fn driver_blanks(work_tree: &Path) -> Result<Env> {
    // --local skips include.path, so read all scopes and filter.
    let out = match git(
        work_tree,
        &[
            "config",
            "--includes",
            "--show-scope",
            "--null",
            "--name-only",
            "--get-regexp",
            r"^filter\.|^merge\..+\.driver$",
        ],
    ) {
        Ok(out) => out,
        // Exit 1 = no match; any other failure must not run without blanks.
        Err(Error::Proc(ProcError::Failed { code: Some(1), .. })) => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let fields: Vec<&str> = out.split('\0').collect();
    let blanks: Vec<(&str, &str)> = fields
        .as_chunks::<2>()
        .0
        .iter()
        .filter_map(|[scope, key]| {
            if key.starts_with("merge.") {
                Some((*key, "false"))
            } else {
                matches!(*scope, "local" | "worktree").then_some((*key, ""))
            }
        })
        .collect();
    let mut env: Env = vec![("GIT_CONFIG_COUNT".into(), blanks.len().to_string().into())];
    for (n, (key, value)) in blanks.into_iter().enumerate() {
        env.push((format!("GIT_CONFIG_KEY_{n}"), key.into()));
        env.push((format!("GIT_CONFIG_VALUE_{n}"), value.into()));
    }
    Ok(env)
}

/// `owner/name` from an origin URL (https, ssh://, scp-like); any host.
pub fn parse_repo(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    let path = match url.split_once("://") {
        Some((_, rest)) => rest.split_once('/')?.1,
        None => url.split_once(':')?.1,
    };
    let mut parts = path.rsplit('/').filter(|s| !s.is_empty());
    let name = parts.next()?;
    let owner = parts.next()?;
    // slug reaches `gh repo view`: a leading `-` would parse as a flag
    let sane = |s: &str| {
        !s.starts_with('-')
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    };
    (sane(owner) && sane(name)).then(|| format!("{owner}/{name}"))
}

/// `owner/name` only when the origin host looks like GitHub (`github-work`
/// ssh aliases included); other hosts must not reach `gh`.
pub fn github_repo(url: &str) -> Option<String> {
    let url = url.trim();
    let authority = match url.split_once("://") {
        Some((_, rest)) => rest.split('/').next()?,
        None => url.split(':').next()?,
    };
    let host = authority.rsplit('@').next()?;
    if host.to_lowercase().contains("github") {
        parse_repo(url)
    } else {
        None
    }
}

pub fn origin_url(repo: &Path) -> Result<Option<String>> {
    match git(repo, &["config", "--get", "remote.origin.url"]) {
        Ok(url) => Ok(Some(url)),
        Err(Error::Proc(ProcError::Failed { .. })) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn origin_repo(repo: &Path) -> Result<Option<String>> {
    Ok(origin_url(repo)?.and_then(|url| parse_repo(&url)))
}

pub fn tags(work_tree: &Path, n: usize) -> Result<Vec<String>> {
    let out = git(work_tree, &["tag", "--sort=-creatordate"])?;
    Ok(out.lines().take(n).map(str::to_owned).collect())
}

pub fn common_dir(path: &Path) -> Result<PathBuf> {
    let out = git(path, &["rev-parse", "--git-common-dir"])?;
    Ok(dunce::canonicalize(path.join(out.trim_end_matches('\n')))?)
}

pub fn last_commit_at(work_tree: &Path) -> Result<Option<i64>> {
    if !verifies(work_tree, "HEAD")? {
        return Ok(None);
    }
    let out = git(work_tree, &["log", "-1", "--format=%ct"])?;
    out.trim()
        .parse()
        .map(Some)
        .map_err(|e| Error::Config(format!("commit time {out:?}: {e}")))
}

pub fn ignored_files(work_tree: &Path) -> Result<Vec<PathBuf>> {
    // ls-files lists each file: status --ignored collapses dirs, hiding .env.
    let out = git(
        work_tree,
        &[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
        ],
    )?;
    Ok(out
        .split('\0')
        .filter(|f| !f.is_empty())
        .map(|p| work_tree.join(p))
        .collect())
}

pub fn is_env_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with(".env"))
}

pub fn default_branch(repo: &Path) -> Result<Option<String>> {
    let origin = git(
        repo,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .ok();
    let origin = origin.as_deref().map(|o| {
        let o = o.trim_end_matches('\n');
        o.strip_prefix("origin/").unwrap_or(o)
    });
    let names = || origin.into_iter().chain(["main", "master"]);
    // Remote first: a local default branch lags until pulled.
    for name in names() {
        if let Some(remote) = remote_branch(repo, name)? {
            return Ok(Some(remote));
        }
    }
    for name in names() {
        if verifies(repo, &format!("refs/heads/{name}"))? {
            return Ok(Some(name.to_owned()));
        }
    }
    Ok(None)
}

pub fn remote_branch(repo: &Path, name: &str) -> Result<Option<String>> {
    Ok(verifies(repo, &format!("refs/remotes/origin/{name}"))?.then(|| format!("origin/{name}")))
}

pub fn local_branch(repo: &Path, name: &str) -> Result<Option<String>> {
    let r = format!("refs/heads/{name}");
    Ok(verifies(repo, &r)?.then_some(r))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeTree {
    SameTip,
    Merged,
    Clean,
    Conflicts,
}

// Same tip = fresh branch, nothing landed yet.
pub fn is_merged(repo: &Path, tip: &str, into: &str) -> Result<bool> {
    Ok(merge_tree(repo, tip, into)? == MergeTree::Merged)
}

// Merged = merging tip into `into` leaves into's tree unchanged: covers squash merges.
pub fn merge_tree(repo: &Path, tip: &str, into: &str) -> Result<MergeTree> {
    let rev = |r: &str| {
        git(repo, &["rev-parse", "--verify", "--end-of-options", r])
            .map(|o| o.trim_end_matches('\n').to_owned())
    };
    let (tip, into) = (rev(tip)?, rev(into)?);
    if tip == into {
        return Ok(MergeTree::SameTip);
    }
    let tree = rev(&format!("{into}^{{tree}}"))?;
    // merge-tree writes objects: send them to a scratch dir, never into the Project.
    let scratch = tempfile::tempdir()?;
    let objects = git(
        repo,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "objects",
        ],
    )?;
    // Alternates are ':'-separated; a double-quoted entry is C-unquoted by git.
    let objects = objects.trim_end_matches('\n');
    let quoted = format!("\"{}\"", objects.replace('\\', "\\\\").replace('"', "\\\""));
    let mut env = driver_blanks(repo)?;
    env.push(("GIT_OBJECT_DIRECTORY".into(), scratch.path().into()));
    env.push(("GIT_ALTERNATE_OBJECT_DIRECTORIES".into(), quoted.into()));
    let args = [
        "merge-tree",
        "--write-tree",
        "--allow-unrelated-histories",
        &into,
        &tip,
    ];
    match git_env(repo, &[], &env, &args) {
        Ok(out) if out.lines().next() == Some(tree.as_str()) => Ok(MergeTree::Merged),
        Ok(_) => Ok(MergeTree::Clean),
        Err(Error::Proc(ProcError::Failed { code: Some(1), .. })) => Ok(MergeTree::Conflicts),
        Err(e) => Err(e),
    }
}

/// Seconds since epoch when the linked Worktree was added.
pub fn created_at(work_tree: &Path) -> Result<i64> {
    let dir = git(work_tree, &["rev-parse", "--absolute-git-dir"])?;
    // commondir is written once by `worktree add`, never rewritten.
    let added =
        std::fs::metadata(Path::new(dir.trim_end_matches('\n')).join("commondir"))?.modified()?;
    let secs = added
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    Ok(i64::try_from(secs).map_err(io::Error::other)?)
}

// ssh prompts on /dev/tty, which GIT_TERMINAL_PROMPT does not cover. The -c also
// overrides a repo-local core.sshCommand; GIT_SSH_COMMAND from the user wins as is.
fn ssh_override(work_tree: &Path) -> Option<String> {
    if env::var_os("GIT_SSH_COMMAND").is_some() {
        return None;
    }
    let base = git(
        work_tree,
        &["config", "--global", "--get", "core.sshCommand"],
    )
    .map(|v| v.trim().to_owned())
    .ok()
    .filter(|v| !v.is_empty())
    .unwrap_or_else(|| "ssh".to_owned());
    Some(format!("core.sshCommand={base} -o BatchMode=yes"))
}

pub fn ls_remote(work_tree: &Path) -> Result<()> {
    let ssh = ssh_override(work_tree);
    // Repo-local config can name programs (uploadpack, credential.helper, askPass...):
    // read only the URL, run ls-remote outside the repo, allow https and ssh only.
    let url = git(work_tree, &["config", "--get", "remote.origin.url"])?;
    let mut config = vec![
        "protocol.allow=never",
        "protocol.https.allow=always",
        "protocol.ssh.allow=always",
    ];
    config.extend(ssh.as_deref());
    let args = ["ls-remote", "--heads", "--", url.trim_end_matches('\n')];
    git_with(Path::new("/"), &config, &args).map(|_| ())
}

pub fn remove_worktree(repo: &Path, path: &Path, force: bool) -> Result<()> {
    // worktree remove runs status inside the worktree: blank its drivers too.
    let blanks = driver_blanks(path)?;
    let path = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "non-UTF-8 worktree path"))?;
    let args = if force {
        vec!["worktree", "remove", "--force", path]
    } else {
        vec!["worktree", "remove", path]
    };
    // git's own clean check must see untracked files a user config hides.
    let config = ["status.showUntrackedFiles=all"];
    git_env(repo, &config, &blanks, &args).map(|_| ())
}

pub fn prune_worktrees(repo: &Path) -> Result<()> {
    git(repo, &["worktree", "prune"]).map(|_| ())
}

fn verifies(repo: &Path, rev: &str) -> Result<bool> {
    match git(
        repo,
        &["rev-parse", "--verify", "--quiet", "--end-of-options", rev],
    ) {
        Ok(_) => Ok(true),
        Err(Error::Proc(ProcError::Failed { .. })) => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn repo_from_origin_url() {
        for (url, want) in [
            ("https://github.com/o/n.git\n", Some("o/n")),
            ("https://github.com/o/n", Some("o/n")),
            ("ssh://git@github.com/o/n.git", Some("o/n")),
            ("git@github.com:o/n.git", Some("o/n")),
            ("https://gitlab.com/g/sub/n.git", Some("sub/n")),
            ("/srv/git/n.git", None),
            ("", None),
            ("git@h:--web/x", None),
            ("https://github.com/-x/y", None),
            ("https://github.com/o/-y", None),
            ("https://github.com/o/a b", None),
            ("git@github.com:o/n.js.git", Some("o/n.js")),
        ] {
            assert_eq!(super::parse_repo(url).as_deref(), want, "{url}");
        }
    }

    #[test]
    fn github_repo_gates_on_host() {
        for (url, want) in [
            ("https://github.com/o/n.git", Some("o/n")),
            ("git@github-work:o/n.git", Some("o/n")),
            ("ssh://git@github.example.com:2222/o/n", Some("o/n")),
            ("https://gitlab.com/o/n.git", None),
            ("git@gitlab.com:github/n.git", None),
            ("https://user@bitbucket.org/o/n", None),
        ] {
            assert_eq!(super::github_repo(url).as_deref(), want, "{url}");
        }
    }
}
