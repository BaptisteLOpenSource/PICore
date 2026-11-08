#[allow(dead_code)] // shared helpers, not all used by every test crate
mod support;

use std::fs;

use picore_core::config::Config;
use picore_core::model::{Project, ProjectId};
use picore_core::prune::{Blocker, Reason, Verdict, assess, remove};
use picore_core::refresh::scan;
use support::TestRepo;

const DAY: i64 = 86_400;

fn project(r: &TestRepo) -> Project {
    scan(&ProjectId(r.path().join(".git").canonicalize().unwrap())).unwrap()
}

fn linked(p: &Project) -> &picore_core::model::Worktree {
    p.linked_worktrees().next().unwrap()
}

fn now() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap()
}

#[test]
fn fresh_branch_is_kept_not_merged() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    let p = project(&r);
    let a = assess(&p, linked(&p), &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Kept(Blocker::NotMerged));
    assert_eq!(a.branch.as_deref(), Some("feat"));
}

#[test]
fn merged_clean_is_prunable() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    r.write("next.txt", "x");
    r.commit("next");
    let p = project(&r);
    let a = assess(&p, linked(&p), &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Prunable(Reason::Merged));
    remove(&p, &a, false).unwrap();
    assert!(!wt.exists());
}

#[test]
fn dirty_is_kept() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    fs::write(wt.join("untracked.txt"), "x").unwrap();
    let p = project(&r);
    let a = assess(&p, linked(&p), &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Kept(Blocker::Changes(1)));
}

#[test]
fn env_file_blocks() {
    let r = TestRepo::new();
    r.write(".gitignore", ".env*\n");
    r.commit("ignore");
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    fs::write(wt.join(".env.local"), "K=V").unwrap();
    r.write("next.txt", "x");
    r.commit("next");
    let p = project(&r);
    let a = assess(&p, linked(&p), &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Kept(Blocker::EnvFile));
}

#[test]
fn missing_dir_is_prunable() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    fs::remove_dir_all(&wt).unwrap();
    let p = project(&r);
    let a = assess(&p, linked(&p), &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Prunable(Reason::Missing));
    remove(&p, &a, false).unwrap();
    let listed = picore_core::git::worktrees(&r.path().join(".git")).unwrap();
    assert!(listed.iter().all(|w| w.path != wt));
}

#[test]
fn main_and_locked_refused_even_forced() {
    let r = TestRepo::new();
    let p = project(&r);
    let main = p.worktrees.iter().find(|w| w.is_main).unwrap();
    let a = assess(&p, main, &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Kept(Blocker::Main));
    assert!(remove(&p, &a, true).is_err());

    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    r.run(&["worktree", "lock", wt.to_str().unwrap()]);
    let p = project(&r);
    let a = assess(&p, linked(&p), &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Kept(Blocker::Locked));
    assert!(remove(&p, &a, true).is_err());
    assert!(wt.is_dir());
}

#[test]
fn force_removes_named_dirty_worktree() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    fs::write(wt.join("untracked.txt"), "x").unwrap();
    let p = project(&r);
    let a = assess(&p, linked(&p), &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Kept(Blocker::Changes(1)));
    assert!(remove(&p, &a, false).is_err());
    assert!(wt.is_dir());
    remove(&p, &a, true).unwrap();
    assert!(!wt.exists());
    r.run(&["rev-parse", "--verify", "refs/heads/feat"]);
}

#[test]
fn stale_flag_uses_stale_days() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    let p = project(&r);
    let w = linked(&p);
    let last = w.last_commit_at.unwrap();
    assert!(
        assess(&p, w, &Config::default(), last + 31 * DAY)
            .unwrap()
            .stale
    );
    assert!(
        !assess(&p, w, &Config::default(), last + 29 * DAY)
            .unwrap()
            .stale
    );
}

#[test]
fn merged_with_ignored_files_is_kept() {
    let r = TestRepo::new();
    r.write(".gitignore", "*.key\n");
    r.commit("ignore");
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    r.write("next.txt", "x");
    r.commit("next");
    fs::write(wt.join("secret.key"), "k").unwrap();
    let p = project(&r);
    let a = assess(&p, linked(&p), &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Kept(Blocker::IgnoredFiles));
}

#[test]
fn merged_with_only_rebuildable_ignored_dir_is_prunable() {
    let r = TestRepo::new();
    r.write(".gitignore", "target/\n");
    r.commit("ignore");
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    r.write("next.txt", "x");
    r.commit("next");
    fs::create_dir(wt.join("target")).unwrap();
    fs::write(wt.join("target/x"), "b").unwrap();
    let p = project(&r);
    let a = assess(&p, linked(&p), &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Prunable(Reason::Merged));
}

#[test]
fn env_file_inside_rebuildable_dir_is_kept() {
    let r = TestRepo::new();
    r.write(".gitignore", "target/\n");
    r.commit("ignore");
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    r.write("next.txt", "x");
    r.commit("next");
    fs::create_dir(wt.join("target")).unwrap();
    fs::write(wt.join("target/.env"), "K=V").unwrap();
    let p = project(&r);
    let a = assess(&p, linked(&p), &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Kept(Blocker::EnvFile));
}

fn set_origin(r: &TestRepo, name: &str, rev: &str) {
    let sha = r.run(&["rev-parse", rev]);
    r.run(&[
        "update-ref",
        &format!("refs/remotes/origin/{name}"),
        sha.trim(),
    ]);
}

fn set_origin_head(r: &TestRepo) {
    r.run(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
    ]);
}

// Branch `feat` with one commit on top of init, checked out in a linked Worktree.
fn feat_worktree(r: &TestRepo) -> std::path::PathBuf {
    r.write("feat.txt", "work");
    r.commit("feat work");
    r.run(&["branch", "feat"]);
    let wt = r.tmp().join("wt");
    r.run(&["worktree", "add", "-q", wt.to_str().unwrap(), "feat"]);
    wt
}

fn verdict_of(r: &TestRepo) -> Verdict {
    let p = project(r);
    assess(&p, linked(&p), &Config::default(), now())
        .unwrap()
        .verdict
}

#[test]
fn merged_into_origin_only_is_prunable() {
    let r = TestRepo::new();
    feat_worktree(&r);
    r.write("more.txt", "x");
    r.commit("more");
    set_origin(&r, "main", "main");
    set_origin_head(&r);
    r.run(&["update-ref", "refs/heads/main", "main~2"]);
    assert_eq!(verdict_of(&r), Verdict::Prunable(Reason::Merged));
}

#[test]
fn squash_merged_is_prunable() {
    let r = TestRepo::new();
    r.write("a.txt", "one");
    r.commit("feat 1");
    r.write("a.txt", "two");
    r.write("b.txt", "b");
    r.commit("feat 2");
    r.run(&["branch", "feat"]);
    r.run(&["update-ref", "refs/heads/main", "main~2"]);
    r.commit("squash feat");
    r.write("later.txt", "x");
    r.commit("later");
    let wt = r.tmp().join("wt");
    r.run(&["worktree", "add", "-q", wt.to_str().unwrap(), "feat"]);
    assert_eq!(verdict_of(&r), Verdict::Prunable(Reason::Merged));
}

#[test]
fn partly_squashed_branch_is_kept() {
    let r = TestRepo::new();
    r.write("a.txt", "one");
    r.commit("feat 1");
    r.write("b.txt", "b");
    r.commit("feat 2");
    r.run(&["branch", "feat"]);
    r.run(&["update-ref", "refs/heads/main", "main~2"]);
    std::fs::remove_file(r.path().join("b.txt")).unwrap();
    r.commit("squash feat 1 only");
    let wt = r.tmp().join("wt");
    r.run(&["worktree", "add", "-q", wt.to_str().unwrap(), "feat"]);
    assert_eq!(verdict_of(&r), Verdict::Kept(Blocker::NotMerged));
}

#[test]
fn merged_into_origin_dev_is_prunable() {
    let r = TestRepo::new();
    feat_worktree(&r);
    r.write("dev.txt", "x");
    r.commit("dev work");
    set_origin(&r, "dev", "main");
    set_origin(&r, "main", "main~2");
    set_origin_head(&r);
    r.run(&["update-ref", "refs/heads/main", "main~2"]);
    assert_eq!(verdict_of(&r), Verdict::Prunable(Reason::Merged));
}

#[test]
fn merged_with_build_output_dirs_is_prunable() {
    let r = TestRepo::new();
    r.write(".gitignore", ".next/\nbuild/\n.gradle/\n");
    r.commit("ignore");
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    r.write("next.txt", "x");
    r.commit("next");
    fs::create_dir_all(wt.join(".next/cache")).unwrap();
    fs::write(wt.join(".next/cache/x"), "b").unwrap();
    fs::create_dir_all(wt.join("android/app/build")).unwrap();
    fs::write(wt.join("android/app/build/out.apk"), "b").unwrap();
    fs::create_dir_all(wt.join("android/.gradle")).unwrap();
    fs::write(wt.join("android/.gradle/lock"), "b").unwrap();
    let p = project(&r);
    let a = assess(&p, linked(&p), &Config::default(), now()).unwrap();
    assert_eq!(a.verdict, Verdict::Prunable(Reason::Merged));
    remove(&p, &a, false).unwrap();
    assert!(!wt.exists());
}
