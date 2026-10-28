#[allow(dead_code)] // shared helpers, not all used by every test crate
mod support;

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
    assert_eq!(h.dirty, 1);
    assert!(h.remote.is_none());
}

#[test]
fn bare_project_has_zero_counts() {
    let r = TestRepo::bare_with_worktree();
    let p = scan(&ProjectId(r.path().canonicalize().unwrap())).unwrap();
    let h = project_health(&p, false).unwrap();
    assert_eq!((h.dirty, h.ahead, h.behind), (0, 0, 0));
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
