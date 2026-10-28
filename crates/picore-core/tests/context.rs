#[cfg(test)]
#[allow(dead_code)] // shared helpers, not all used by every test crate
mod support;

use std::fs;

use picore_core::context::{context, safe_read, section};
use picore_core::model::ProjectId;
use picore_core::refresh::scan;
use support::TestRepo;

#[test]
fn symlink_outside_root_refused() {
    let r = TestRepo::new();
    let secret = r.tmp().join("secret.txt");
    fs::write(&secret, "hidden").unwrap();
    fs::remove_file(r.path().join("README.md")).unwrap();
    std::os::unix::fs::symlink(&secret, r.path().join("README.md")).unwrap();
    assert!(safe_read(r.path(), "README.md").is_err());
}

#[test]
fn non_whitelisted_refused() {
    let r = TestRepo::new();
    r.write(".env", "TOKEN=1");
    assert!(safe_read(r.path(), ".env").is_err());
}

#[test]
fn context_outlines_whitelisted_files() {
    let r = TestRepo::new();
    r.write("README.md", "# Title\nintro\n## Install\nx\n");
    let p = scan(&ProjectId(r.path().join(".git").canonicalize().unwrap())).unwrap();
    let ctx = context(&p).unwrap();
    assert_eq!(ctx.files.len(), 1);
    assert_eq!(ctx.files[0].file, "README.md");
    assert_eq!(ctx.files[0].headings.len(), 2);
    let s = section(&p, "README.md", "Install").unwrap();
    assert_eq!(s.text.trim(), "x");
}

#[test]
fn bare_project_has_no_files() {
    let r = TestRepo::bare_with_worktree();
    let p = scan(&ProjectId(r.path().to_path_buf())).unwrap();
    assert!(context(&p).unwrap().files.is_empty());
    assert!(section(&p, "README.md", "Install").is_err());
}

#[test]
fn non_utf8_file_is_lossy_not_fatal() {
    let r = TestRepo::new();
    fs::write(r.path().join("README.md"), [0xff, 0xfe, b'#', b' ', b'A']).unwrap();
    let p = scan(&ProjectId(r.path().join(".git").canonicalize().unwrap())).unwrap();
    let ctx = context(&p).unwrap();
    assert_eq!(ctx.files.len(), 1);
    assert_eq!(ctx.files[0].file, "README.md");
}

#[test]
fn bare_section_non_whitelisted_is_config_error() {
    let r = TestRepo::bare_with_worktree();
    let p = scan(&ProjectId(r.path().to_path_buf())).unwrap();
    assert!(matches!(
        section(&p, ".env", "Install"),
        Err(picore_core::error::Error::Config(_))
    ));
}
