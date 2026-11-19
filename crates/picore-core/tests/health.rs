#[allow(dead_code)] // shared helpers, not all used by every test crate
mod support;

use std::fs;
use std::time::{Duration, Instant};

use picore_core::health::{Status, project_health};
use picore_core::model::ProjectId;
use picore_core::refresh::scan;
use support::TestRepo;

fn project(r: &TestRepo) -> picore_core::model::Project {
    scan(&ProjectId(
        dunce::canonicalize(r.path().join(".git")).unwrap(),
    ))
    .unwrap()
}

#[test]
fn dirty_count_and_no_remote_check_by_default() {
    let r = TestRepo::new();
    r.write("new.txt", "x");
    let h = project_health(&project(&r), false);
    assert_eq!(h.worktrees[0].dirty, Some(1));
    assert!(h.remote.is_none());
}

#[test]
fn bare_project_has_no_worktree_rows() {
    let r = TestRepo::bare_with_worktree();
    let p = scan(&ProjectId(dunce::canonicalize(r.path()).unwrap())).unwrap();
    assert!(project_health(&p, false).worktrees.is_empty());
}

#[test]
fn dirty_linked_worktree_reported_in_its_own_row() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    fs::write(wt.join("agent.txt"), "x").unwrap();
    let h = project_health(&project(&r), false);
    let rows: Vec<_> = h
        .worktrees
        .iter()
        .map(|w| (w.branch.as_deref(), w.dirty))
        .collect();
    assert_eq!(rows, [(Some("main"), Some(0)), (Some("feat"), Some(1))]);
    assert_eq!(h.worktrees[1].path, dunce::canonicalize(&wt).unwrap());
}

#[test]
fn no_upstream_is_none_not_zero() {
    let r = TestRepo::new();
    let h = project_health(&project(&r), false);
    let w = &h.worktrees[0];
    assert_eq!(
        (w.ahead, w.behind, w.upstream.as_deref()),
        (None, None, None)
    );
}

#[test]
fn tracked_branch_in_sync_is_some_zero() {
    let r = TestRepo::new();
    let origin = r.tmp().join("origin.git");
    r.run(&["clone", "-q", "--bare", s(r.path()), s(&origin)]);
    r.run(&["remote", "add", "origin", s(&origin)]);
    r.run(&["fetch", "-q", "origin"]);
    r.run(&["branch", "-q", "--set-upstream-to", "origin/main"]);
    r.commit("local");
    let h = project_health(&project(&r), false);
    let w = &h.worktrees[0];
    assert_eq!((w.ahead, w.behind), (Some(1), Some(0)));
    assert_eq!(w.upstream.as_deref(), Some("origin/main"));
}

fn s(p: &std::path::Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn unreachable_remote_is_unknown_fast() {
    let r = TestRepo::new();
    r.run(&["remote", "add", "origin", "/picore-no-such/x.git"]);
    let t = Instant::now();
    let h = project_health(&project(&r), true);
    assert!(t.elapsed() < Duration::from_secs(7));
    assert_eq!(h.remote.unwrap().status, Status::Unknown);
}

#[test]
fn missing_and_broken_worktrees_are_row_errors() {
    let r = TestRepo::new();
    let gone = r.tmp().join("gone");
    let broken = r.tmp().join("broken");
    r.add_worktree(&gone, "gone");
    r.add_worktree(&broken, "broken");
    let p = project(&r);
    fs::remove_dir_all(&gone).unwrap();
    fs::write(broken.join(".git"), "gitdir: /picore-no-such\n").unwrap();
    let h = project_health(&p, false);
    let mut rows: Vec<_> = h
        .worktrees
        .iter()
        .map(|w| (w.branch.as_deref().unwrap(), w.dirty, w.error.is_some()))
        .collect();
    rows.sort();
    assert_eq!(
        rows,
        [
            ("broken", None, true),
            ("gone", None, true),
            ("main", Some(0), false)
        ]
    );
    let gone = h.worktrees.iter().find(|w| w.path.ends_with("gone"));
    let json = serde_json::to_value(gone.unwrap()).unwrap();
    assert!(json["dirty"].is_null() && json["ahead"].is_null(), "{json}");
    assert!(
        json["error"].as_str().unwrap().contains("missing"),
        "{json}"
    );
}
