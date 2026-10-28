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

    // Plumbing only: `git commit` would run the global hooks, which reject commits on main.
    pub fn commit(&self, msg: &str) {
        self.run(&["add", "-A"]);
        let tree = self.run(&["write-tree"]);
        let parent = process::run(
            "git",
            &self.path,
            &["rev-parse", "--verify", "-q", "HEAD"],
            TIMEOUT,
        )
        .ok()
        .map(|p| p.trim().to_owned());
        let mut args = vec!["commit-tree", tree.trim(), "-m", msg];
        if let Some(p) = parent.as_deref() {
            args.extend(["-p", p]);
        }
        let sha = self.run(&args);
        self.run(&["update-ref", "HEAD", sha.trim()]);
    }

    pub fn add_worktree(&self, dir: &Path, branch: &str) {
        self.run(&["worktree", "add", "-q", "-b", branch, dir.to_str().unwrap()]);
    }

    pub fn run(&self, args: &[&str]) -> String {
        process::run("git", &self.path, args, TIMEOUT)
            .unwrap_or_else(|e| panic!("git {args:?}: {e}"))
    }
}
