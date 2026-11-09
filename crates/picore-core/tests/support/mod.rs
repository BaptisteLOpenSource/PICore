use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use picore_core::process::{self, TIMEOUT};
use tempfile::TempDir;

pub struct TestRepo {
    _root: TempDir,
    root: PathBuf,
    path: PathBuf,
}

impl TestRepo {
    pub fn unborn() -> Self {
        let root = TempDir::new().unwrap();
        let root_path = root.path().to_path_buf();
        let path = root_path.join("repo");
        fs::create_dir(&path).unwrap();
        let r = Self {
            _root: root,
            root: root_path,
            path,
        };
        r.run(&["init", "-q", "-b", "main"]);
        r.run(&["config", "user.name", "PICore Test"]);
        r.run(&["config", "user.email", "test@picore.invalid"]);
        r.run(&["config", "commit.gpgsign", "false"]);
        r
    }

    pub fn new() -> Self {
        let r = Self::unborn();
        r.write("README.md", "init");
        r.commit("init");
        r
    }

    pub fn bare_with_worktree() -> Self {
        let mut r = Self::new();
        let bare = r.root.join("bare.git");
        r.run(&[
            "clone",
            "-q",
            "--bare",
            r.path.to_str().unwrap(),
            bare.to_str().unwrap(),
        ]);
        r.path = bare;
        r
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn tmp(&self) -> &Path {
        &self.root
    }

    pub fn write(&self, file: &str, content: &str) {
        fs::write(self.path.join(file), content).unwrap();
    }

    pub fn commit(&self, msg: &str) {
        self.commit_in(&self.path, msg, None);
    }

    pub fn commit_in(&self, dir: &Path, msg: &str, date: Option<&str>) {
        commit_in(dir, msg, date);
    }

    pub fn add_worktree(&self, dir: &Path, branch: &str) {
        self.run(&["worktree", "add", "-q", "-b", branch, dir.to_str().unwrap()]);
    }

    pub fn run(&self, args: &[&str]) -> String {
        self.run_in(&self.path, args)
    }

    pub fn run_in(&self, dir: &Path, args: &[&str]) -> String {
        process::run("git", dir, args, TIMEOUT).unwrap_or_else(|e| panic!("git {args:?}: {e}"))
    }
}

// Plumbing only: `git commit` would run the global hooks, which reject commits on main.
// `dir` may be a linked Worktree: HEAD there moves its branch. `date` = committer date.
pub fn commit_in(dir: &Path, msg: &str, date: Option<&str>) {
    let git = |args: &[&str]| {
        process::run("git", dir, args, TIMEOUT).unwrap_or_else(|e| panic!("git {args:?}: {e}"))
    };
    git(&["add", "-A"]);
    let tree = git(&["write-tree"]);
    let parent = process::run(
        "git",
        dir,
        &["rev-parse", "--verify", "-q", "HEAD"],
        TIMEOUT,
    )
    .ok()
    .map(|p| p.trim().to_owned());
    let mut args = vec![
        "-c",
        "user.name=PICore Test",
        "-c",
        "user.email=test@picore.invalid",
        "commit-tree",
        tree.trim(),
        "-m",
        msg,
    ];
    if let Some(p) = parent.as_deref() {
        args.extend(["-p", p]);
    }
    let env: Vec<_> = date
        .map(|d| ("GIT_COMMITTER_DATE", OsStr::new(d)))
        .into_iter()
        .collect();
    let sha = process::run_env("git", dir, &args, &env, TIMEOUT)
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    git(&["update-ref", "HEAD", sha.trim()]);
}
