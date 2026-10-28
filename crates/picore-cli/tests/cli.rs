use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use picore_core::process::{self, TIMEOUT};
use tempfile::TempDir;

struct Sandbox {
    dir: TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Sandbox {
            dir: TempDir::new().unwrap(),
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn repo(&self, rel: &str) -> PathBuf {
        let p = self.path().join(rel);
        fs::create_dir_all(&p).unwrap();
        process::run("git", &p, &["init", "-q", "-b", "main"], TIMEOUT).unwrap();
        p
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_picore"));
        cmd.env("PICORE_CONFIG_DIR", self.path().join("config"))
            .env("PICORE_DATA_DIR", self.path().join("data"));
        cmd
    }

    fn picore(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap()
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn json_envelope_on_list() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    assert!(
        sb.picore(&["root", "add", s(&sb.path().join("code"))])
            .status
            .success()
    );
    assert!(sb.picore(&["index"]).status.success());

    let v = json(&sb.picore(&["list", "--json"]));
    assert_eq!(v["schema"], 1);
    assert_eq!(v["data"][0]["name"], "a");
}

#[test]
fn piped_output_has_no_mascot() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    assert!(
        sb.picore(&["root", "add", s(&sb.path().join("code"))])
            .status
            .success()
    );

    let out = sb.picore(&["index"]);
    assert!(out.status.success());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("(o>"));
}

#[test]
fn index_without_roots_non_tty_fails() {
    let sb = Sandbox::new();

    let out = sb.picore(&["--json", "index"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json(&out)["error"]["code"], "no_roots");
}

#[test]
fn root_remove_then_index_drops_projects() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    sb.repo("work/b");
    let code = sb.path().join("code");
    assert!(sb.picore(&["root", "add", s(&code)]).status.success());
    assert!(
        sb.picore(&["root", "add", s(&sb.path().join("work"))])
            .status
            .success()
    );
    assert!(sb.picore(&["index"]).status.success());
    assert_eq!(
        json(&sb.picore(&["list", "--json"]))["data"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    assert!(sb.picore(&["root", "remove", s(&code)]).status.success());
    assert!(sb.picore(&["index"]).status.success());
    let v = json(&sb.picore(&["list", "--json"]));
    assert_eq!(v["data"].as_array().unwrap().len(), 1);
    assert_eq!(v["data"][0]["name"], "b");
}

#[test]
fn index_without_git_fails_cleanly() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    assert!(
        sb.picore(&["root", "add", s(&sb.path().join("code"))])
            .status
            .success()
    );

    let out = sb
        .command()
        .env("PATH", "")
        .args(["--json", "index"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json(&out)["error"]["code"], "git");
}
