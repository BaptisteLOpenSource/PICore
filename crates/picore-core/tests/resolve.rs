#[allow(dead_code)] // shared helpers, not all used by every test crate
mod support;

use std::fs;

use picore_core::model::{Project, ProjectId};
use picore_core::process::{self, TIMEOUT};
use picore_core::refresh::scan;
use picore_core::resolve::{Resolution, resolve};
use support::TestRepo;

fn project(path: &std::path::Path) -> Project {
    scan(&ProjectId(dunce::canonicalize(path.join(".git")).unwrap())).unwrap()
}

#[test]
fn subdir_of_linked_worktree_resolves_to_it() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "80-feat");
    let sub = wt.join("a/b");
    fs::create_dir_all(&sub).unwrap();
    let ps = [project(r.path())];
    let Resolution::One(t) = resolve(&ps, sub.to_str().unwrap()) else {
        panic!("expected One");
    };
    let w = t.worktree.unwrap();
    assert_eq!(w.path, dunce::canonicalize(&wt).unwrap());
    assert_eq!(w.issue, Some(80));
    let Resolution::One(t) = resolve(&ps, r.path().to_str().unwrap()) else {
        panic!("expected One");
    };
    assert!(t.worktree.unwrap().is_main);
}

#[test]
fn name_at_issue_resolves_linked_worktree() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "80-feat");
    let ps = [project(r.path())];
    let Resolution::One(t) = resolve(&ps, "repo@80") else {
        panic!("expected One");
    };
    assert_eq!(t.worktree.unwrap().path, dunce::canonicalize(&wt).unwrap());
}

#[test]
fn nested_repo_longest_prefix_wins() {
    let outer = TestRepo::new();
    let inner = outer.path().join("inner");
    fs::create_dir(&inner).unwrap();
    process::run("git", &inner, &["init", "-q", "-b", "main"], TIMEOUT).unwrap();
    let ps = [project(outer.path()), project(&inner)];
    let Resolution::One(t) = resolve(&ps, inner.to_str().unwrap()) else {
        panic!("expected One");
    };
    assert_eq!(t.project.name, "inner");
}

#[test]
fn path_outside_every_project_is_none() {
    let r = TestRepo::new();
    let ps = [project(r.path())];
    assert_eq!(resolve(&ps, "/"), Resolution::None);
}

#[test]
fn scan_reads_origin_repo() {
    let r = TestRepo::new();
    assert_eq!(project(r.path()).repo, None);
    process::run(
        "git",
        r.path(),
        &["remote", "add", "origin", "git@github.com:o/n.git"],
        TIMEOUT,
    )
    .unwrap();
    assert_eq!(project(r.path()).repo.as_deref(), Some("o/n"));
}

#[test]
fn bare_project_dir_is_first_linked_worktree() {
    let r = TestRepo::bare_with_worktree();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    let ps = [project_at(r.path())];
    let Resolution::One(t) = resolve(&ps, "bare") else {
        panic!("expected One");
    };
    assert_eq!(t.dir(), Some(dunce::canonicalize(&wt).unwrap().as_path()));
}

fn project_at(git_dir: &std::path::Path) -> Project {
    scan(&ProjectId(dunce::canonicalize(git_dir).unwrap())).unwrap()
}

#[test]
fn bare_git_dir_path_resolves_to_project() {
    let r = TestRepo::bare_with_worktree();
    let ps = [project_at(r.path())];
    let git_dir = dunce::canonicalize(r.path()).unwrap();
    let Resolution::One(t) = resolve(&ps, git_dir.to_str().unwrap()) else {
        panic!("expected One");
    };
    assert_eq!(t.project.id.0, git_dir);
    assert!(t.worktree.is_none());
}
