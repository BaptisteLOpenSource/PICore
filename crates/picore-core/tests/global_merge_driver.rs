// Own test binary: it points GIT_CONFIG_GLOBAL at a temp file for the whole process.
#[allow(dead_code)] // shared helpers, not all used by every test crate
mod support;

use std::fs;

use picore_core::git;
use support::TestRepo;

#[test]
fn is_merged_runs_no_global_merge_driver() {
    let r = TestRepo::new();
    r.write("a.txt", "base");
    r.commit("base");
    r.write("a.txt", "theirs");
    r.commit("theirs");
    r.run(&["branch", "other"]);
    r.run(&["update-ref", "refs/heads/main", "main~1"]);
    r.write("a.txt", "ours");
    r.commit("ours");
    // `true` keeps ours: a run driver would fake a merged tree.
    let global = r.tmp().join("global.cfg");
    fs::write(&global, "[merge \"keep\"]\n\tdriver = true\n").unwrap();
    fs::write(r.path().join(".git/info/attributes"), "*.txt merge=keep\n").unwrap();
    // SAFETY: only test in this binary, set before any thread reads the env.
    unsafe { std::env::set_var("GIT_CONFIG_GLOBAL", &global) };
    assert!(!git::is_merged(r.path(), "other", "main").unwrap());
}
