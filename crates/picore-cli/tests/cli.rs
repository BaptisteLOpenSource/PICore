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

fn git(dir: &Path, args: &[&str]) -> String {
    process::run("git", dir, args, TIMEOUT).unwrap()
}

// Plumbing only: `git commit` would run the global hooks, which reject commits on main.
fn commit(repo: &Path, msg: &str) {
    fs::write(repo.join("f.txt"), msg).unwrap();
    git(repo, &["add", "-A"]);
    let tree = git(repo, &["write-tree"]);
    let parent = process::run(
        "git",
        repo,
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
    let sha = git(repo, &args).trim().to_owned();
    git(repo, &["update-ref", "HEAD", &sha]);
}

// `merged` is an ancestor of main (prunable), `dirty` has an untracked file (kept).
fn merged_and_dirty(sb: &Sandbox) -> (PathBuf, PathBuf) {
    let repo = sb.repo("code/a");
    commit(&repo, "one");
    let merged = sb.path().join("wt/merged");
    let dirty = sb.path().join("wt/dirty");
    git(
        &repo,
        &["worktree", "add", "-q", "-b", "merged", s(&merged), "main"],
    );
    git(
        &repo,
        &["worktree", "add", "-q", "-b", "dirty", s(&dirty), "main"],
    );
    fs::write(dirty.join("wip.txt"), "x").unwrap();
    commit(&repo, "two");
    (merged, dirty)
}

fn add_code_root(sb: &Sandbox) {
    assert!(
        sb.picore(&["root", "add", s(&sb.path().join("code"))])
            .status
            .success()
    );
    assert!(sb.picore(&["index"]).status.success());
}

#[test]
fn open_ambiguous_non_tty_lists_candidates() {
    let sb = Sandbox::new();
    sb.repo("code/c1/api");
    sb.repo("code/c2/api");
    add_code_root(&sb);

    let out = sb.picore(&["--json", "open", "api"]);
    assert_eq!(out.status.code(), Some(3));
    let v = json(&out);
    assert_eq!(v["error"]["code"], "ambiguous");
    assert_eq!(
        v["error"]["candidates"],
        serde_json::json!(["c1/api", "c2/api"])
    );
}

#[test]
fn open_json_uses_editor_config() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    add_code_root(&sb);
    let cfg = sb.path().join("config/config.toml");
    let text = fs::read_to_string(&cfg).unwrap();
    fs::write(&cfg, format!("{text}editor = \"true\"\n")).unwrap();

    let out = sb.picore(&["--json", "open", "a"]);
    assert!(out.status.success());
    assert_eq!(json(&out)["data"]["editor"], serde_json::json!(["true"]));
}

#[test]
fn prune_is_dry_run_by_default() {
    let sb = Sandbox::new();
    let (merged, _dirty) = merged_and_dirty(&sb);
    add_code_root(&sb);

    let out = sb.picore(&["--json", "prune"]);
    assert!(out.status.success());
    assert_eq!(json(&out)["data"]["dry_run"], true);
    assert!(merged.is_dir());
}

#[test]
fn prune_dry_run_shows_reason() {
    let sb = Sandbox::new();
    let (merged, _dirty) = merged_and_dirty(&sb);
    let repo = sb.path().join("code/a");
    let gone = sb.path().join("wt/gone");
    git(
        &repo,
        &["worktree", "add", "-q", "-b", "gone", s(&gone), "main"],
    );
    fs::remove_dir_all(&gone).unwrap();
    add_code_root(&sb);

    let out = sb.picore(&["prune"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    let line = |path: &Path| {
        text.lines()
            .find(|l| l.contains(s(path)))
            .unwrap()
            .to_owned()
    };
    assert!(line(&gone).ends_with("Prunable(Missing)"), "{text}");
    assert!(line(&merged).ends_with("Prunable(Merged)"), "{text}");

    let out = sb.picore(&["--json", "prune"]);
    let rows = json(&out)["data"]["pruned"].clone();
    let verdict = |path: &Path| {
        rows.as_array()
            .unwrap()
            .iter()
            .find(|r| r["path"] == s(path))
            .unwrap()["verdict"]
            .clone()
    };
    assert_eq!(verdict(&gone), serde_json::json!({"Prunable": "Missing"}));
    assert_eq!(verdict(&merged), serde_json::json!({"Prunable": "Merged"}));
}

#[test]
fn prune_yes_removes_merged_only() {
    let sb = Sandbox::new();
    let (merged, dirty) = merged_and_dirty(&sb);
    add_code_root(&sb);

    let out = sb.picore(&["--json", "prune", "--yes"]);
    assert!(out.status.success());
    assert!(!merged.exists());
    assert!(dirty.is_dir());
}

#[test]
fn prune_named_dirty_needs_force() {
    let sb = Sandbox::new();
    let (_merged, dirty) = merged_and_dirty(&sb);
    add_code_root(&sb);

    let out = sb.picore(&["--json", "prune", "dirty", "--yes"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json(&out)["error"]["code"], "refused");
    assert!(dirty.is_dir());

    let out = sb.picore(&["--json", "prune", "dirty", "--yes", "--force"]);
    assert_eq!(json(&out)["error"]["code"], "refused");
    assert!(dirty.is_dir());

    let target = dirty.to_str().unwrap();
    let out = sb.picore(&["--json", "prune", target, "--yes", "--force"]);
    assert!(out.status.success());
    assert!(!dirty.exists());
}

#[test]
fn status_json_has_machine_checks() {
    let sb = Sandbox::new();

    let out = sb
        .command()
        .env("PATH", "")
        .args(["--json", "status"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = json(&out);
    let names: Vec<&str> = v["data"]["machine"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["git", "ssh-agent", "gh"]);
}

#[test]
fn open_has_short_alias() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    add_code_root(&sb);
    let cfg = sb.path().join("config/config.toml");
    let text = fs::read_to_string(&cfg).unwrap();
    fs::write(&cfg, format!("{text}editor = \"true\"\n")).unwrap();
    assert!(sb.picore(&["--json", "o", "a"]).status.success());
}

#[test]
fn completions_emit_script_for_each_shell() {
    let sb = Sandbox::new();
    for shell in ["bash", "zsh", "fish"] {
        let out = sb.picore(&["completions", shell]);
        assert!(out.status.success(), "{shell}");
        let script = String::from_utf8(out.stdout).unwrap();
        assert!(script.contains("picore"), "{shell}");
    }
}

#[test]
fn man_page_is_roff() {
    let sb = Sandbox::new();
    let out = sb.picore(&["man"]);
    assert!(out.status.success());
    let page = String::from_utf8(out.stdout).unwrap();
    assert!(page.contains(".TH"));
}
