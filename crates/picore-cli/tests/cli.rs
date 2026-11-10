#[allow(dead_code)] // shared helpers, not all used here
#[path = "../../picore-core/tests/support/mod.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

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
fn index_never_touches_another_schema_version_file() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    let data = sb.path().join("data");
    fs::create_dir_all(&data).unwrap();
    // Stand-in for a v2 index another installed picore still uses.
    let old = data.join("picore.db");
    fs::write(&old, "v2 index").unwrap();
    assert!(
        sb.picore(&["root", "add", s(&sb.path().join("code"))])
            .status
            .success()
    );
    assert!(sb.picore(&["index"]).status.success());
    assert_eq!(fs::read_to_string(&old).unwrap(), "v2 index");
    let current = format!("picore-v{}.db", picore_core::store::VERSION);
    assert!(data.join(current).is_file());
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

fn commit(repo: &Path, msg: &str) {
    fs::write(repo.join("f.txt"), msg).unwrap();
    support::commit_in(repo, msg, None);
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
    // Own commit after creation, else the Worktree reads Fresh.
    commit(&merged, "merged work");
    git(&repo, &["merge", "-q", "--ff-only", "merged"]);
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
fn list_shows_product() {
    let sb = Sandbox::new();
    sb.repo("code/shop/api");
    sb.repo("code/b");
    add_code_root(&sb);
    let cfg = sb.path().join("config/config.toml");
    let text = fs::read_to_string(&cfg).unwrap();
    let shop = sb.path().join("code/shop");
    fs::write(
        &cfg,
        format!("{text}[products.shop]\npath = \"{}\"\n", s(&shop)),
    )
    .unwrap();

    let v = json(&sb.picore(&["list", "--json"]));
    let product = |name: &str| {
        v["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .unwrap()["product"]
            .clone()
    };
    assert_eq!(product("api"), "shop");
    assert!(product("b").is_null());
    let table = String::from_utf8(sb.picore(&["list"]).stdout).unwrap();
    assert!(table.starts_with("NAME") && table.contains("PRODUCT") && table.contains("shop"));
}

#[test]
fn list_survives_bad_product_config() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    add_code_root(&sb);
    let cfg = sb.path().join("config/config.toml");
    let text = fs::read_to_string(&cfg).unwrap();
    fs::write(&cfg, format!("{text}[products.p]\npath = \"rel\"\n")).unwrap();

    let out = sb.picore(&["list", "--json"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("products hidden"));
    assert_eq!(json(&out)["data"][0]["name"], "a");
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
        let r = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["path"] == s(path))
            .unwrap()
            .clone();
        (
            r["verdict"].clone(),
            r["reason"].clone(),
            r["removed"].clone(),
        )
    };
    let row = |v: &str, r: &str| {
        (
            serde_json::json!(v),
            serde_json::json!(r),
            serde_json::json!(false),
        )
    };
    assert_eq!(verdict(&gone), row("prunable", "missing"));
    assert_eq!(verdict(&merged), row("prunable", "merged"));
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
fn status_by_worktree_subdir_reports_each_worktree_and_remote() {
    let sb = Sandbox::new();
    let a = sb.repo("code/a");
    support::commit_in(&a, "i", None);
    let git = |args: &[&str]| process::run("git", &a, args, TIMEOUT).unwrap();
    git(&["remote", "add", "origin", "/picore-no-such/x.git"]);
    let wt = sb.path().join("wt");
    git(&["worktree", "add", "-q", "-b", "feat", s(&wt)]);
    fs::create_dir(wt.join("sub")).unwrap();
    fs::write(wt.join("agent.txt"), "x").unwrap();
    add_code_root(&sb);

    let out = sb.picore(&["status", "--remote", s(&wt.join("sub"))]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    let wt = fs::canonicalize(&wt).unwrap();
    assert!(text.contains("a: remote Unknown"), "{text}");
    assert!(
        text.contains(&format!("a {}: dirty 1 no upstream", wt.display())),
        "{text}"
    );
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

#[test]
fn broken_pipe_exits_quietly() {
    let sb = Sandbox::new();
    for args in [&["completions", "bash"][..], &["man"][..]] {
        let mut child = sb
            .command()
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        drop(child.stdout.take());
        let out = child.wait_with_output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!err.contains("panicked"), "{args:?}: {err}");
        assert!(!err.contains("Broken pipe"), "{args:?}: {err}");
    }
}

#[test]
fn gui_without_binary_fails() {
    let sb = Sandbox::new();
    let out = sb
        .command()
        .arg("gui")
        .env("PATH", sb.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&out.stderr).contains("picore-gui not found on PATH"));
}

#[cfg(unix)]
#[test]
fn gui_spawns_detached_and_prints_nothing() {
    use std::os::unix::fs::PermissionsExt;

    let sb = Sandbox::new();
    let bin = sb.path().join("picore-gui");
    fs::write(&bin, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    let out = sb
        .command()
        .arg("gui")
        .env("PATH", sb.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
}

#[test]
fn exit_codes_are_stable() {
    let sb = Sandbox::new();
    sb.repo("code/c1/api");
    sb.repo("code/c2/api");

    assert_eq!(sb.picore(&["index"]).status.code(), Some(1));
    assert_eq!(sb.picore(&["--bogus"]).status.code(), Some(2));
    add_code_root(&sb);
    assert_eq!(sb.picore(&["list"]).status.code(), Some(0));
    assert_eq!(sb.picore(&["open", "zzz-none"]).status.code(), Some(4));
    assert_eq!(sb.picore(&["open", "api"]).status.code(), Some(3));
}

fn update_with_receipts(sb: &Sandbox) -> Output {
    sb.command()
        .args(["update", "--json"])
        .env("AXOUPDATER_CONFIG_PATH", sb.path().join("receipts"))
        .output()
        .unwrap()
}

#[test]
fn update_without_receipt_fails_cleanly() {
    let sb = Sandbox::new();
    let out = update_with_receipts(&sb);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json(&out)["error"]["code"], "not_installer");
}

#[test]
fn update_refuses_binary_outside_receipt_prefix() {
    let sb = Sandbox::new();
    let receipts = sb.path().join("receipts");
    fs::create_dir_all(&receipts).unwrap();
    let receipt = serde_json::json!({
        "install_prefix": s(&sb.path().join("elsewhere")),
        "binaries": ["picore"],
        "source": {"release_type": "github", "owner": "o", "name": "n", "app_name": "picore"},
        "version": "0.1.0",
        "provider": {"source": "cargo-dist", "version": "0.32.0"},
    });
    fs::write(receipts.join("picore-receipt.json"), receipt.to_string()).unwrap();

    let out = update_with_receipts(&sb);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json(&out)["error"]["code"], "not_installer");
}

#[test]
fn path_prints_project_dir_and_json() {
    let sb = Sandbox::new();
    let repo = sb.repo("code/a");
    add_code_root(&sb);

    let out = sb.picore(&["path", "a"]);
    assert!(out.status.success());
    let printed = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        fs::canonicalize(printed.trim()).unwrap(),
        fs::canonicalize(&repo).unwrap()
    );
    let v = json(&sb.picore(&["--json", "path", "a"]));
    assert_eq!(v["data"]["path"].as_str().unwrap(), printed.trim());
    assert_eq!(sb.picore(&["path", "zzz-none"]).status.code(), Some(4));
}

#[test]
fn path_ambiguous_non_tty_fails() {
    let sb = Sandbox::new();
    sb.repo("code/c1/api");
    sb.repo("code/c2/api");
    add_code_root(&sb);
    assert_eq!(sb.picore(&["path", "api"]).status.code(), Some(3));
}

#[test]
fn init_prints_pcd_per_shell() {
    let sb = Sandbox::new();
    let text = |sh: &str| String::from_utf8(sb.picore(&["init", sh]).stdout).unwrap();
    assert!(text("fish").starts_with("function pcd"));
    assert!(text("fish").contains("(picore path $argv); or return"));
    for sh in ["zsh", "bash"] {
        let t = text(sh);
        assert!(t.starts_with("pcd()") && t.contains(r#"picore path "$@")" || return"#));
    }
    assert!(!sb.picore(&["init", "nu"]).status.success());
}

#[test]
fn info_without_origin_reports_no_github() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    add_code_root(&sb);
    let v = json(&sb.picore(&["--json", "info", "a"]));
    let d = &v["data"];
    assert_eq!(d["name"], "a");
    assert!(d["github"].is_null() && d["repo"].is_null());
    assert!(d["github_error"].is_string());
    assert!(d["tags"].as_array().unwrap().is_empty());
    assert!(sb.picore(&["info", "a"]).status.success());
}

#[test]
fn list_json_is_the_shared_view() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    add_code_root(&sb);
    let v = json(&sb.picore(&["list", "--json"]));
    let row = v["data"][0].as_object().unwrap();
    let mut keys: Vec<&str> = row.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "branch",
            "id",
            "name",
            "product",
            "repo",
            "techs",
            "worktrees"
        ]
    );
    assert_eq!(row["worktrees"], 1);
}

#[test]
fn list_sees_new_repo_without_index() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    add_code_root(&sb);
    sb.repo("code/b");
    let v = json(&sb.picore(&["list", "--json"]));
    assert_eq!(v["data"].as_array().unwrap().len(), 2);
}

#[test]
fn status_reports_missing_worktree_row_and_fails() {
    let sb = Sandbox::new();
    let (merged, _dirty) = merged_and_dirty(&sb);
    add_code_root(&sb);
    fs::remove_dir_all(&merged).unwrap();
    let out = sb.picore(&["--json", "status", "a"]);
    assert_eq!(out.status.code(), Some(1));
    let v = json(&out);
    let rows = v["data"]["projects"][0]["worktrees"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    let gone = rows.iter().find(|r| r["path"] == s(&merged)).unwrap();
    assert!(
        gone["dirty"].is_null() && gone["error"].is_string(),
        "{gone}"
    );
    assert!(rows.iter().filter(|r| r["error"].is_null()).count() == 2);
}

#[test]
fn worktrees_accepts_path_like_status() {
    let sb = Sandbox::new();
    let (merged, _dirty) = merged_and_dirty(&sb);
    add_code_root(&sb);
    let out = sb.picore(&["--json", "worktrees", s(&merged)]);
    assert!(out.status.success());
    assert_eq!(json(&out)["data"].as_array().unwrap().len(), 3);
}

#[cfg(unix)]
#[test]
fn prune_yes_reports_every_row_and_fails_on_any_error() {
    use std::os::unix::fs::PermissionsExt;

    let sb = Sandbox::new();
    let (merged, _dirty) = merged_and_dirty(&sb);
    let repo = sb.path().join("code/a");
    let stuck = sb.path().join("wt/stuck");
    git(
        &repo,
        &["worktree", "add", "-q", "-b", "stuck", s(&stuck), "main"],
    );
    fs::create_dir(stuck.join("ro")).unwrap();
    commit(&stuck, "stuck work");
    git(&repo, &["merge", "-q", "--ff-only", "stuck"]);
    fs::create_dir(stuck.join("ro/x")).unwrap();
    fs::write(stuck.join("ro/x/f"), "").unwrap();
    git(&stuck, &["add", "-A"]);
    commit(&stuck, "nested");
    git(&repo, &["merge", "-q", "--ff-only", "stuck"]);
    commit(&repo, "three");
    add_code_root(&sb);
    let ro = stuck.join("ro/x");
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();

    let out = sb.picore(&["--json", "prune", "--yes"]);
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = json(&out);
    let rows = v["data"]["pruned"].as_array().unwrap();
    let row = |p: &Path| rows.iter().find(|r| r["path"] == s(p)).unwrap().clone();
    assert_eq!(row(&merged)["removed"], true);
    assert!(row(&merged)["error"].is_null());
    assert_eq!(row(&stuck)["removed"], false);
    assert!(row(&stuck)["error"].is_string());
    assert!(!merged.exists());
}

#[test]
fn refresh_of_indexed_root_runs_no_git() {
    let sb = Sandbox::new();
    sb.repo("code/a");
    add_code_root(&sb);
    // No git on PATH: any discovery or scan git spawn would fail the command.
    let out = sb
        .command()
        .env("PATH", "")
        .args(["--json", "list"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json(&out)["data"][0]["name"], "a");
}
