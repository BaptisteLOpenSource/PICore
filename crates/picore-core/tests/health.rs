#[allow(dead_code)] // shared helpers, not all used by every test crate
mod support;

use std::fs;
use std::time::{Duration, Instant};

use picore_core::health::{Status, project_health};
use picore_core::model::ProjectId;
use picore_core::refresh::scan;
use support::TestRepo;

fn project(r: &TestRepo) -> picore_core::model::Project {
    scan(&ProjectId(r.path().join(".git").canonicalize().unwrap())).unwrap()
}

#[test]
fn dirty_count_and_no_remote_check_by_default() {
    let r = TestRepo::new();
    r.write("new.txt", "x");
    let h = project_health(&project(&r), false).unwrap();
    assert_eq!(h.worktrees[0].dirty, 1);
    assert!(h.remote.is_none());
}

#[test]
fn bare_project_has_no_worktree_rows() {
    let r = TestRepo::bare_with_worktree();
    let p = scan(&ProjectId(r.path().canonicalize().unwrap())).unwrap();
    assert!(project_health(&p, false).unwrap().worktrees.is_empty());
}

#[test]
fn dirty_linked_worktree_reported_in_its_own_row() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    fs::write(wt.join("agent.txt"), "x").unwrap();
    let h = project_health(&project(&r), false).unwrap();
    let rows: Vec<_> = h
        .worktrees
        .iter()
        .map(|w| (w.branch.as_deref(), w.dirty))
        .collect();
    assert_eq!(rows, [(Some("main"), 0), (Some("feat"), 1)]);
    assert_eq!(h.worktrees[1].path, wt.canonicalize().unwrap());
}

#[test]
fn no_upstream_is_none_not_zero() {
    let r = TestRepo::new();
    let h = project_health(&project(&r), false).unwrap();
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
    let h = project_health(&project(&r), false).unwrap();
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
    let h = project_health(&project(&r), true).unwrap();
    assert!(t.elapsed() < Duration::from_secs(7));
    assert_eq!(h.remote.unwrap().status, Status::Unknown);
}
