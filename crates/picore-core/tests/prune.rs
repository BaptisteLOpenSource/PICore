#[allow(dead_code)] // shared helpers, not all used by every test crate
mod support;

use std::fs;

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
    let a = assess(&p, linked(&p), 30, now()).unwrap();
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
    let a = assess(&p, linked(&p), 30, now()).unwrap();
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
    let a = assess(&p, linked(&p), 30, now()).unwrap();
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
    let a = assess(&p, linked(&p), 30, now()).unwrap();
    assert_eq!(a.verdict, Verdict::Kept(Blocker::EnvFile));
}

#[test]
fn missing_dir_is_prunable() {
    let r = TestRepo::new();
    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    fs::remove_dir_all(&wt).unwrap();
    let p = project(&r);
    let a = assess(&p, linked(&p), 30, now()).unwrap();
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
    let a = assess(&p, main, 30, now()).unwrap();
    assert_eq!(a.verdict, Verdict::Kept(Blocker::Main));
    assert!(remove(&p, &a, true).is_err());

    let wt = r.tmp().join("wt");
    r.add_worktree(&wt, "feat");
    r.run(&["worktree", "lock", wt.to_str().unwrap()]);
    let p = project(&r);
    let a = assess(&p, linked(&p), 30, now()).unwrap();
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
    let a = assess(&p, linked(&p), 30, now()).unwrap();
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
    assert!(assess(&p, w, 30, last + 31 * DAY).unwrap().stale);
    assert!(!assess(&p, w, 30, last + 29 * DAY).unwrap().stale);
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
    let a = assess(&p, linked(&p), 30, now()).unwrap();
    assert_eq!(a.verdict, Verdict::Kept(Blocker::IgnoredFiles));
}
