#[cfg(test)]
#[allow(dead_code)] // shared helpers, not all used by every test crate
mod support;

use std::fs;

use picore_core::config::Config;
use picore_core::context::{ProjectContext, safe_read, section};
use picore_core::model::{Project, ProjectId};
use picore_core::refresh::scan;
use picore_core::view;
use support::TestRepo;

fn context(p: &Project) -> picore_core::error::Result<ProjectContext> {
    with_config(p, &Config::default())
}

fn with_config(p: &Project, cfg: &Config) -> picore_core::error::Result<ProjectContext> {
    picore_core::context::context(p, &view::of(p, std::slice::from_ref(p), cfg))
}

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

#[test]
fn in_root_symlink_to_other_file_refused() {
    let r = TestRepo::new();
    r.write(".env", "TOKEN=1");
    fs::remove_file(r.path().join("README.md")).unwrap();
    std::os::unix::fs::symlink(".env", r.path().join("README.md")).unwrap();
    assert!(safe_read(r.path(), "README.md").is_err());
}

#[test]
fn fifo_refused_without_blocking() {
    let r = TestRepo::new();
    fs::remove_file(r.path().join("README.md")).unwrap();
    let ok = std::process::Command::new("mkfifo")
        .arg(r.path().join("README.md"))
        .status()
        .unwrap();
    assert!(ok.success());
    assert!(safe_read(r.path(), "README.md").is_err());
}

#[test]
fn read_capped_at_one_mib() {
    let r = TestRepo::new();
    r.write("README.md", &"x".repeat(2 << 20));
    let md = safe_read(r.path(), "README.md").unwrap().unwrap();
    assert_eq!(md.len(), 1 << 20);
}

#[test]
fn context_carries_product_from_config() {
    let r = TestRepo::new();
    let p = scan(&ProjectId(r.path().join(".git").canonicalize().unwrap())).unwrap();
    let f = r.tmp().join("config.toml");
    let root = r.tmp().canonicalize().unwrap();
    fs::write(
        &f,
        format!("[products.demo]\npath = \"{}\"\n", root.display()),
    )
    .unwrap();
    let cfg = Config::load(&f).unwrap();
    assert_eq!(
        with_config(&p, &cfg).unwrap().product.as_deref(),
        Some("demo")
    );
}
