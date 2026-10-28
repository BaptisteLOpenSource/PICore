#[allow(dead_code)] // shared helpers, not all used by every test crate
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use picore_core::config::Config;
use picore_core::discovery::{discover, suggest_roots};
use picore_core::model::{ProjectId, project_name};
use picore_core::process::{self, TIMEOUT};
use support::TestRepo;
use tempfile::TempDir;

fn init(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    process::run("git", dir, &["init", "-q"], TIMEOUT).unwrap();
}

fn found(roots: &[PathBuf], max_depth: usize) -> Vec<ProjectId> {
    discover(roots, max_depth, &Config::default().ignore_dirs).unwrap()
}

#[test]
fn finds_repo_worktree_dedupes_to_one_project() {
    let r = TestRepo::new();
    r.add_worktree(&r.tmp().join("wt"), "feat");
    let ids = found(&[r.tmp().to_path_buf()], 4);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].0, r.path().join(".git").canonicalize().unwrap());
}

#[test]
fn finds_bare_repo_named_dot_bare() {
    let r = TestRepo::new();
    let scan = r.tmp().join("scan");
    let bare = scan.join("x/.bare");
    fs::create_dir_all(bare.parent().unwrap()).unwrap();
    r.run(&[
        "clone",
        "-q",
        "--bare",
        r.path().to_str().unwrap(),
        bare.to_str().unwrap(),
    ]);
    let wt = scan.join("x/main");
    process::run(
        "git",
        &bare,
        &["worktree", "add", "-q", "-b", "feat", wt.to_str().unwrap()],
        TIMEOUT,
    )
    .unwrap();
    let ids = found(&[scan], 4);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].0, bare.canonicalize().unwrap());
    assert_eq!(project_name(&ids[0]), "x");
}

#[test]
fn skips_nested_repo_and_ignore_dirs() {
    let d = TempDir::new().unwrap();
    let scan = d.path();
    init(&scan.join("repo"));
    init(&scan.join("repo/sub"));
    init(&scan.join("node_modules/dep"));
    let ids = found(&[scan.to_path_buf()], 4);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].0, scan.join("repo/.git").canonicalize().unwrap());
    assert_eq!(project_name(&ids[0]), "repo");
}

#[test]
fn skips_submodule() {
    let outer = TestRepo::new();
    let sub = TestRepo::new();
    outer.run(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        sub.path().to_str().unwrap(),
        "sub",
    ]);
    let ids = found(&[outer.tmp().to_path_buf()], 4);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].0, outer.path().join(".git").canonicalize().unwrap());
}

#[test]
fn respects_max_depth() {
    let d = TempDir::new().unwrap();
    init(&d.path().join("a/b/c/d/repo"));
    let roots = [d.path().to_path_buf()];
    assert!(found(&roots, 4).is_empty());
    assert_eq!(found(&roots, 5).len(), 1);
}

#[test]
fn root_itself_is_repo() {
    let r = TestRepo::new();
    let ids = found(&[r.path().to_path_buf()], 4);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].0, r.path().join(".git").canonicalize().unwrap());
}

#[test]
fn symlink_loop_terminates() {
    let d = TempDir::new().unwrap();
    init(&d.path().join("repo"));
    std::os::unix::fs::symlink(d.path(), d.path().join("loop")).unwrap();
    let ids = found(&[d.path().to_path_buf()], 4);
    assert_eq!(ids.len(), 1);
}

#[test]
fn suggest_roots_lists_code_dir_with_repo() {
    let home = TempDir::new().unwrap();
    init(&home.path().join("code/a"));
    fs::create_dir_all(home.path().join("dev")).unwrap();
    assert_eq!(suggest_roots(home.path()), vec![home.path().join("code")]);
}
