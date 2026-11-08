mod support;

use std::fs;
use std::path::PathBuf;

use picore_core::git;
use picore_core::process::{self, TIMEOUT};
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
    let ignored = git::ignored_files(r.path()).unwrap();
    assert_eq!(ignored.len(), 1);
    assert!(git::is_env_file(&ignored[0]));
}

#[test]
fn default_branch_falls_back_to_origin_when_local_missing() {
    let r = TestRepo::new();
    r.run(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    r.run(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
    ]);
    let sha = r.run(&["rev-parse", "HEAD"]);
    r.run(&["symbolic-ref", "HEAD", "refs/heads/work"]);
    r.run(&["update-ref", "refs/heads/work", sha.trim()]);
    r.run(&["update-ref", "-d", "refs/heads/main"]);
    r.write("next.txt", "x");
    r.commit("next");
    let default = git::default_branch(r.path()).unwrap().unwrap();
    assert_eq!(default, "origin/main");
    assert!(!git::is_strict_ancestor(r.path(), "work", &default).unwrap());
    assert!(git::is_strict_ancestor(r.path(), "origin/main", "work").unwrap());
}

#[test]
fn fsmonitor_in_repo_config_is_not_run() {
    let r = TestRepo::new();
    let marker = r.tmp().join("marker");
    let hook = r.tmp().join("fsm.sh");
    fs::write(&hook, format!("#!/bin/sh\ntouch {}\n", marker.display())).unwrap();
    r.run(&["config", "core.fsmonitor", hook.to_str().unwrap()]);
    fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    git::status(r.path()).unwrap();
    git::ignored_files(r.path()).unwrap();
    assert!(!marker.exists());
}

#[test]
fn rev_starting_with_dash_is_not_an_option() {
    let r = TestRepo::new();
    assert!(git::is_strict_ancestor(r.path(), "main", "--help").is_err());
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

#[test]
fn repo_local_uploadpack_is_not_run() {
    let r = TestRepo::new();
    let marker = r.tmp().join("marker");
    let hook = r.tmp().join("pack.sh");
    fs::write(&hook, format!("#!/bin/sh\ntouch {}\n", marker.display())).unwrap();
    fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    r.run(&["remote", "add", "origin", r.path().to_str().unwrap()]);
    r.run(&["config", "remote.origin.uploadpack", hook.to_str().unwrap()]);
    r.run(&["config", "protocol.file.allow", "always"]);
    assert!(git::ls_remote(r.path()).is_err());
    assert!(!marker.exists());
}

fn assert_filter_not_run(bind: impl Fn(&TestRepo, &str)) {
    let r = TestRepo::new();
    let (marker, clean) = marker_script(&r);
    r.write("a.txt", "x");
    r.commit("add a");
    r.write(".gitattributes", "*.txt filter=mark\n");
    bind(&r, &clean);
    r.write("a.txt", "x");
    git::status(r.path()).unwrap();
    assert!(!marker.exists());
}

#[test]
fn repo_local_filter_driver_is_not_run_by_status() {
    assert_filter_not_run(|r, clean| {
        r.run(&["config", "filter.mark.clean", clean]);
    });
}

#[test]
fn included_filter_driver_is_not_run_by_status() {
    assert_filter_not_run(|r, clean| {
        let inc = r.tmp().join("inc.cfg");
        fs::write(&inc, format!("[filter \"mark\"]\n\tclean = {clean}\n")).unwrap();
        r.run(&["config", "include.path", inc.to_str().unwrap()]);
    });
}

fn marker_script(r: &TestRepo) -> (PathBuf, String) {
    let marker = r.tmp().join("marker");
    let clean = r.tmp().join("clean.sh");
    fs::write(
        &clean,
        format!("#!/bin/sh\ntouch {}\ncat\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(&clean, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    (marker, clean.to_str().unwrap().to_owned())
}

#[test]
fn status_does_not_rewrite_index_on_stale_entries() {
    let r = TestRepo::new();
    let index = r.path().join(".git/index");
    let before = fs::read(&index).unwrap();
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(100);
    fs::File::options()
        .write(true)
        .open(r.path().join("README.md"))
        .unwrap()
        .set_modified(old)
        .unwrap();
    git::status(r.path()).unwrap();
    assert_eq!(fs::read(&index).unwrap(), before);
}

#[test]
fn submodule_filter_driver_is_not_run_by_status() {
    let r = TestRepo::new();
    let (marker, clean) = marker_script(&r);
    let sub = r.path().join("sub");
    fs::create_dir(&sub).unwrap();
    let in_sub = |args: &[&str]| process::run("git", &sub, args, TIMEOUT).unwrap();
    in_sub(&["init", "-q"]);
    fs::write(sub.join(".gitattributes"), "*.txt filter=mark\n").unwrap();
    fs::write(sub.join("f.txt"), "x").unwrap();
    in_sub(&["add", "-A"]);
    let tree = in_sub(&["write-tree"]);
    let sha = in_sub(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t.invalid",
        "commit-tree",
        tree.trim(),
        "-m",
        "sub",
    ]);
    in_sub(&["update-ref", "HEAD", sha.trim()]);
    r.run(&["add", "sub"]);
    r.commit("add sub");
    in_sub(&["config", "filter.mark.clean", &clean]);
    fs::write(sub.join("f.txt"), "x").unwrap();
    git::status(r.path()).unwrap();
    assert!(!marker.exists());
}

#[test]
fn repo_local_filter_driver_is_not_run_by_worktree_remove() {
    let r = TestRepo::new();
    let (marker, clean) = marker_script(&r);
    r.write(".gitattributes", "*.txt filter=mark\n");
    r.write("a.txt", "x");
    r.commit("add a");
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    r.run(&["config", "filter.mark.clean", &clean]);
    fs::write(wt.join("a.txt"), "x").unwrap();
    git::remove_worktree(r.path(), &wt, false).unwrap();
    assert!(!marker.exists());
}

#[test]
fn tags_newest_first_capped() {
    let r = TestRepo::new();
    for i in 0..7 {
        r.run(&["tag", "-a", &format!("v{i}"), "-m", "t"]);
    }
    let t = git::tags(r.path(), 5).unwrap();
    assert_eq!(t.len(), 5);
    assert!(git::tags(TestRepo::unborn().path(), 5).unwrap().is_empty());
}
