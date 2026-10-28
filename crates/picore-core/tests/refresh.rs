mod support;

use std::fs;
use std::thread;
use std::time::Duration;

use picore_core::config::Config;
use picore_core::model::ProjectId;
use picore_core::refresh::{Outcome, index_all, refresh, stamp};
use picore_core::store::Index;
use support::TestRepo;
use tempfile::TempDir;

fn repo_id(r: &TestRepo) -> ProjectId {
    ProjectId(r.path().join(".git").canonicalize().unwrap())
}

fn open(dir: &TempDir) -> Index {
    Index::open(&dir.path().join("picore.db")).unwrap()
}

fn bump(last: &mut i64, id: &ProjectId, action: &str) {
    thread::sleep(Duration::from_millis(10));
    let now = stamp(id).unwrap();
    assert!(now > *last, "{action}: stamp {now} not above {last}");
    *last = now;
}

#[test]
fn refresh_is_fresh_when_nothing_changed() {
    let r = TestRepo::new();
    let db = TempDir::new().unwrap();
    let index = open(&db);
    let id = repo_id(&r);
    assert_eq!(refresh(&index, &id).unwrap(), Outcome::Updated);
    assert_eq!(refresh(&index, &id).unwrap(), Outcome::Fresh);
}

#[test]
fn stamp_moves_on_commit_checkout_branch_and_worktree() {
    let r = TestRepo::new();
    let id = repo_id(&r);
    let mut last = stamp(&id).unwrap();

    r.write("a.txt", "a");
    r.commit("a");
    bump(&mut last, &id, "commit");

    r.run(&["checkout", "-q", "-b", "x"]);
    bump(&mut last, &id, "checkout -b");

    r.run(&["branch", "y"]);
    bump(&mut last, &id, "branch");

    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    bump(&mut last, &id, "worktree add");

    r.run(&["worktree", "remove", wt.to_str().unwrap()]);
    bump(&mut last, &id, "worktree remove");
}

#[test]
fn gone_project_is_removed() {
    let r = TestRepo::new();
    let db = TempDir::new().unwrap();
    let index = open(&db);
    let id = repo_id(&r);
    refresh(&index, &id).unwrap();
    assert!(index.get(&id).unwrap().is_some());

    fs::remove_dir_all(r.path()).unwrap();
    assert_eq!(refresh(&index, &id).unwrap(), Outcome::Gone);
    assert!(index.get(&id).unwrap().is_none());
}

#[test]
fn index_all_counts_and_retains() {
    let a = TestRepo::new();
    let b = TestRepo::new();
    let db = TempDir::new().unwrap();
    let index = open(&db);
    let cfg = Config {
        roots: vec![a.tmp().to_path_buf(), b.tmp().to_path_buf()],
        ..Config::default()
    };

    let first = index_all(&index, &cfg).unwrap();
    assert_eq!((first.found, first.updated, first.removed), (2, 2, 0));

    fs::remove_dir_all(b.tmp()).unwrap();
    let second = index_all(&index, &cfg).unwrap();
    assert_eq!((second.found, second.removed), (1, 1));
    assert_eq!(index.all().unwrap().len(), 1);
}

#[test]
fn unborn_repo_indexes() {
    let r = TestRepo::unborn();
    let db = TempDir::new().unwrap();
    let index = open(&db);
    let cfg = Config {
        roots: vec![r.tmp().to_path_buf()],
        ..Config::default()
    };

    let report = index_all(&index, &cfg).unwrap();
    assert_eq!(report.found, 1);
    let project = index.get(&repo_id(&r)).unwrap().unwrap();
    assert_eq!(project.worktrees.len(), 1);
    assert_eq!(project.worktrees[0].last_commit_at, None);
    assert_eq!(project.worktrees[0].head, None);
    assert_eq!(project.branch.as_deref(), Some("main"));
}

#[test]
fn worktree_count_excludes_main() {
    let r = TestRepo::new();
    let db = TempDir::new().unwrap();
    let index = open(&db);
    r.add_worktree(&r.tmp().join("wt"), "feat");
    refresh(&index, &repo_id(&r)).unwrap();

    let project = index.get(&repo_id(&r)).unwrap().unwrap();
    assert_eq!(project.worktrees.len(), 2);
    assert_eq!(project.linked_worktrees().count(), 1);
}

#[test]
fn bare_layout_has_no_main_work_tree() {
    let r = TestRepo::bare_with_worktree();
    r.add_worktree(&r.tmp().join("wt"), "feat");
    let db = TempDir::new().unwrap();
    let index = open(&db);
    let id = ProjectId(r.path().canonicalize().unwrap());
    refresh(&index, &id).unwrap();

    let project = index.get(&id).unwrap().unwrap();
    assert_eq!(project.work_tree, None);
    assert_eq!(project.branch, None);
    assert_eq!(project.worktrees.len(), 1);
    assert!(!project.worktrees[0].is_main);
}

#[test]
fn missing_worktree_dir_still_indexes() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    fs::remove_dir_all(&wt).unwrap();
    let db = TempDir::new().unwrap();
    let index = open(&db);
    let id = repo_id(&r);
    refresh(&index, &id).unwrap();

    let project = index.get(&id).unwrap().unwrap();
    assert_eq!(project.worktrees.len(), 2);
    assert_eq!(
        project.linked_worktrees().next().unwrap().last_commit_at,
        None
    );
}

#[test]
fn refresh_stores_techs_of_main_work_tree() {
    let r = TestRepo::new();
    let db = TempDir::new().unwrap();
    let index = open(&db);
    r.write(
        "Cargo.toml",
        "[package]\nname = \"x\"\n\n[dependencies]\ntauri = \"2\"\n",
    );
    refresh(&index, &repo_id(&r)).unwrap();

    let project = index.get(&repo_id(&r)).unwrap().unwrap();
    assert_eq!(project.techs, ["rust", "tauri"]);
}
