mod support;

use std::fs;

use picore_core::git;
use support::TestRepo;

#[test]
fn worktree_list_parses_main_and_linked_with_space() {
    let r = TestRepo::new();
    let wt = r.tmp().join("my wt é");
    r.add_worktree(&wt, "feat");
    let list = git::worktrees(r.path()).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1].path, wt.canonicalize().unwrap());
    assert_eq!(list[1].branch.as_deref(), Some("feat"));
}

#[test]
fn status_counts_changes_and_untracked() {
    let r = TestRepo::new();
    r.write("a.txt", "x");
    r.write("README.md", "changed");
    let s = git::status(r.path()).unwrap();
    assert_eq!((s.branch.as_deref(), s.changes), (Some("main"), 2));
}

#[test]
fn unborn_repo_has_no_last_commit() {
    assert_eq!(
        git::last_commit_at(TestRepo::unborn().path()).unwrap(),
        None
    );
}

#[test]
fn strict_ancestor_excludes_same_tip() {
    let r = TestRepo::new();
    r.run(&["branch", "fresh"]);
    assert!(!git::is_strict_ancestor(r.path(), "fresh", "main").unwrap());
    r.commit("next");
    assert!(git::is_strict_ancestor(r.path(), "fresh", "main").unwrap());
}

#[test]
fn env_file_detected_when_ignored() {
    let r = TestRepo::new();
    r.write(".gitignore", ".env*\n");
    r.commit("ignore");
    r.write(".env.local", "K=V");
    assert_eq!(git::ignored_env_files(r.path()).unwrap().len(), 1);
}

#[test]
fn parse_status_reads_ahead_behind() {
    let z = "# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -1\0? x\0";
    let s = git::parse_status(z);
    assert_eq!(
        (s.ahead, s.behind, s.changes, s.upstream.as_deref()),
        (2, 1, 1, Some("origin/main"))
    );
}

#[test]
fn parse_status_skips_rename_origin_path() {
    let z =
        "# branch.head main\x002 R. N... 100644 100644 100644 a a R100 new name\0old name\0? x\0";
    let s = git::parse_status(z);
    assert_eq!(s.changes, 2);
}

#[test]
fn parse_worktree_list_reads_flags_and_strips_ref() {
    let z = "worktree /r\0HEAD abc\0branch refs/heads/main\0\0worktree /w\0HEAD def\0detached\0locked why\0\0";
    let list = git::parse_worktree_list(z);
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].branch.as_deref(), Some("main"));
    assert!(list[1].detached && list[1].locked && !list[1].prunable);
    assert_eq!(list[1].branch, None);
}

#[test]
fn common_dir_from_linked_worktree_is_shared_git_dir() {
    let r = TestRepo::bare_with_worktree();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    assert_eq!(
        git::common_dir(&wt).unwrap(),
        r.path().canonicalize().unwrap()
    );
}

#[test]
fn default_branch_is_main_and_none_when_unborn() {
    assert_eq!(
        git::default_branch(TestRepo::new().path()).unwrap(),
        Some("main".into())
    );
    assert_eq!(
        git::default_branch(TestRepo::unborn().path()).unwrap(),
        None
    );
}

#[test]
fn remove_worktree_deletes_directory() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    git::remove_worktree(r.path(), &wt, false).unwrap();
    assert!(!wt.exists());
    assert_eq!(git::worktrees(r.path()).unwrap().len(), 1);
}

#[test]
fn prune_drops_worktree_whose_directory_vanished() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    fs::remove_dir_all(&wt).unwrap();
    git::prune_worktrees(r.path()).unwrap();
    assert_eq!(git::worktrees(r.path()).unwrap().len(), 1);
}
