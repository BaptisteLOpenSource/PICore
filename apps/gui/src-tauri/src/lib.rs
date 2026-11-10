use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use picore_core::config::{Config, Paths};
use picore_core::context::{self, ProjectContext};
use picore_core::health::{self, Check, ProjectHealth};
use picore_core::prune::{self, Assessment};
use picore_core::refresh;
use picore_core::resolve::resolve;
use picore_core::store::Index;
use picore_core::view::{self, ProjectView};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize)]
pub struct Listed {
    #[serde(flatten)]
    pub view: ProjectView,
    pub health: ProjectHealth,
}

#[derive(Serialize)]
pub struct Overview {
    pub projects: Vec<Listed>,
    pub products: BTreeMap<String, PathBuf>,
    pub roots: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct Detail {
    pub context: ProjectContext,
    pub health: ProjectHealth,
    pub prune: Vec<Assessment>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub fn overview(paths: &Paths, discover: bool) -> Result<Overview, String> {
    let (cfg, bad) = Config::load_or_default(&paths.config_file);
    let loaded = bad.is_none();
    let mut warnings: Vec<String> = bad
        .map(|e| format!("{e}: products hidden"))
        .into_iter()
        .collect();
    let index = Index::open(&paths.index_file).map_err(err)?;
    if discover && loaded {
        match refresh::index_all(&index, &cfg) {
            Ok(report) => warnings.extend(
                report
                    .failed
                    .iter()
                    .map(|f| format!("{}: {}", f.id.0.display(), f.error)),
            ),
            Err(e) => warnings.push(format!("discovery: {e}")),
        }
    }
    let (projects, failed) = refresh::refresh_all(&index, &cfg).map_err(err)?;
    warnings.extend(
        failed
            .iter()
            .map(|(id, e)| format!("{}: {e}", id.0.display())),
    );
    if projects.is_empty() {
        warnings.push("no projects indexed: add a scan root".into());
    }
    let projects = projects
        .iter()
        .zip(view::list(&projects, &cfg))
        .map(|(p, view)| Listed {
            view,
            health: health::project_health(p, false),
        })
        .collect();
    let products = cfg
        .products
        .iter()
        .map(|(name, p)| (name.clone(), p.path.clone()))
        .collect();
    Ok(Overview {
        projects,
        products,
        roots: cfg.roots,
        warnings,
    })
}

/// `id` = `ProjectView::id`, resolved like everywhere else.
pub fn detail(paths: &Paths, id: PathBuf) -> Result<Detail, String> {
    let all = Index::open(&paths.index_file)
        .and_then(|i| i.all())
        .map_err(err)?;
    let query = id.to_string_lossy();
    let project = resolve(&all, &query)
        .into_target(&query)
        .map_err(err)?
        .project;
    let (cfg, _) = Config::load_or_default(&paths.config_file);
    Ok(Detail {
        context: context::context(project, &view::of(project, &all, &cfg)).map_err(err)?,
        health: health::project_health(project, false),
        prune: prune::assess_project(project, &cfg),
    })
}

pub fn add_product(paths: &Paths, name: &str, path: &Path) -> Result<(), String> {
    let mut cfg = Config::load(&paths.config_file).map_err(err)?;
    cfg.add_product(name, path).map_err(err)?;
    cfg.save(&paths.config_file).map_err(err)
}

pub fn remove_product(paths: &Paths, name: &str) -> Result<(), String> {
    let mut cfg = Config::load(&paths.config_file).map_err(err)?;
    if !cfg.remove_product(name) {
        return Err(format!("unknown product {name}"));
    }
    cfg.save(&paths.config_file).map_err(err)
}

pub fn add_root(paths: &Paths, path: &Path) -> Result<(), String> {
    let mut cfg = Config::load(&paths.config_file).map_err(err)?;
    cfg.add_root(path).map_err(err)?;
    cfg.save(&paths.config_file).map_err(err)
}

pub fn remove_root(paths: &Paths, path: &Path) -> Result<(), String> {
    let mut cfg = Config::load(&paths.config_file).map_err(err)?;
    if !cfg.remove_root(path) {
        return Err(format!("unknown scan root {}", path.display()));
    }
    cfg.save(&paths.config_file).map_err(err)
}

mod commands {
    use super::*;

    #[tauri::command(async)]
    pub fn overview(paths: State<'_, Paths>, discover: bool) -> Result<crate::Overview, String> {
        crate::overview(&paths, discover)
    }

    #[tauri::command(async)]
    pub fn detail(paths: State<'_, Paths>, id: PathBuf) -> Result<crate::Detail, String> {
        crate::detail(&paths, id)
    }

    #[tauri::command(async)]
    pub fn machine() -> Vec<Check> {
        health::machine_health()
    }

    #[tauri::command(async)]
    pub fn add_product(paths: State<'_, Paths>, name: String, path: PathBuf) -> Result<(), String> {
        crate::add_product(&paths, &name, &path)
    }

    #[tauri::command(async)]
    pub fn remove_product(paths: State<'_, Paths>, name: String) -> Result<(), String> {
        crate::remove_product(&paths, &name)
    }

    #[tauri::command(async)]
    pub fn add_root(paths: State<'_, Paths>, path: PathBuf) -> Result<(), String> {
        crate::add_root(&paths, &path)
    }

    #[tauri::command(async)]
    pub fn remove_root(paths: State<'_, Paths>, path: PathBuf) -> Result<(), String> {
        crate::remove_root(&paths, &path)
    }
}

fn watch_config(app: &AppHandle, config_file: &Path) -> Result<RecommendedWatcher, String> {
    let handle = app.clone();
    watch(config_file, move || {
        let _ = handle.emit("config-changed", ());
    })
}

fn watch(
    config_file: &Path,
    on_change: impl Fn() + Send + 'static,
) -> Result<RecommendedWatcher, String> {
    let dir = config_file.parent().ok_or("config path has no parent")?;
    fs::create_dir_all(dir).map_err(err)?;
    let target = config_file.to_path_buf();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        // inotify reports opens too: our own config reads would loop refresh.
        if matches!(event, Ok(e) if !e.kind.is_access() && e.paths.contains(&target)) {
            on_change();
        }
    })
    .map_err(err)?;
    // Non-recursive on the directory: editors and `save` replace the file, so a file watch goes stale.
    watcher
        .watch(dir, RecursiveMode::NonRecursive)
        .map_err(err)?;
    Ok(watcher)
}

pub fn run() {
    let paths = Paths::from_env().expect("picore paths");
    let watched = paths.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(paths)
        .setup(move |app| {
            match watch_config(app.handle(), &watched.config_file) {
                Ok(watcher) => {
                    app.manage(Mutex::new(watcher));
                }
                Err(e) => eprintln!("config watcher disabled: {e}"),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::overview,
            commands::detail,
            commands::machine,
            commands::add_product,
            commands::remove_product,
            commands::add_root,
            commands::remove_root,
        ])
        .run(tauri::generate_context!())
        .expect("tauri app");
}

#[cfg(test)]
#[allow(dead_code)] // shared helpers, not all used here
#[path = "../../../../crates/picore-core/tests/support/mod.rs"]
mod support;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use picore_core::config::{Config, Paths};
    use picore_core::process::TIMEOUT;

    use super::*;

    fn repo(p: &Path) {
        fs::create_dir_all(p).unwrap();
        picore_core::process::run("git", p, &["init", "-q", "-b", "main"], TIMEOUT).unwrap();
    }

    fn setup() -> (tempfile::TempDir, Paths, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        repo(&src.join("prod/app"));
        repo(&src.join("solo"));
        let paths = Paths {
            config_file: tmp.path().join("config.toml"),
            index_file: tmp.path().join("picore.db"),
        };
        let mut cfg = Config::default();
        cfg.add_root(&src).unwrap();
        cfg.save(&paths.config_file).unwrap();
        (tmp, paths, src)
    }

    fn find<'a>(ov: &'a Overview, name: &str) -> &'a Listed {
        ov.projects.iter().find(|l| l.view.name == name).unwrap()
    }

    #[test]
    fn overview_lists_projects_with_products() {
        let (_tmp, paths, src) = setup();
        add_product(&paths, "prod", &src.join("prod")).unwrap();
        let ov = overview(&paths, true).unwrap();
        assert_eq!(ov.projects.len(), 2);
        assert_eq!(find(&ov, "app").view.product.as_deref(), Some("prod"));
        assert_eq!(find(&ov, "solo").view.product, None);
        assert!(ov.products.contains_key("prod"));
    }

    #[test]
    fn add_product_rejects_duplicate_and_keeps_file() {
        let (_tmp, paths, src) = setup();
        add_product(&paths, "prod", &src.join("prod")).unwrap();
        let before = fs::read_to_string(&paths.config_file).unwrap();
        let err = add_product(&paths, "prod", &src.join("solo")).unwrap_err();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(fs::read_to_string(&paths.config_file).unwrap(), before);
    }

    #[test]
    fn broken_config_keeps_index_and_is_never_saved() {
        let (_tmp, paths, _src) = setup();
        overview(&paths, true).unwrap();
        fs::write(&paths.config_file, "roots = 3").unwrap();
        let ov = overview(&paths, true).unwrap();
        assert_eq!(ov.projects.len(), 2);
        assert!(!ov.warnings.is_empty());
        assert!(add_product(&paths, "prod", Path::new("/")).is_err());
        assert_eq!(fs::read_to_string(&paths.config_file).unwrap(), "roots = 3");
    }

    #[test]
    fn add_root_discovers_new_projects() {
        let (tmp, paths, _src) = setup();
        let other = tmp.path().join("other");
        repo(&other.join("extra"));
        add_root(&paths, &other).unwrap();
        overview(&paths, true).unwrap();
        let ov = overview(&paths, false).unwrap();
        assert_eq!(ov.projects.len(), 3);
        assert!(ov.roots.contains(&dunce::canonicalize(&other).unwrap()));
    }

    #[test]
    fn add_root_rejects_missing_folder() {
        let (tmp, paths, _src) = setup();
        assert!(add_root(&paths, &tmp.path().join("gone")).is_err());
    }

    #[test]
    fn remove_root_drops_its_projects() {
        let (_tmp, paths, src) = setup();
        overview(&paths, true).unwrap();
        remove_root(&paths, &src).unwrap();
        let ov = overview(&paths, true).unwrap();
        assert!(ov.projects.is_empty());
        assert!(ov.roots.is_empty());
        let err = remove_root(&paths, &src).unwrap_err();
        assert!(err.starts_with("unknown scan root"), "{err}");
    }

    #[test]
    fn remove_product_then_unknown() {
        let (_tmp, paths, src) = setup();
        add_product(&paths, "prod", &src.join("prod")).unwrap();
        overview(&paths, true).unwrap();
        remove_product(&paths, "prod").unwrap();
        assert_eq!(
            find(&overview(&paths, false).unwrap(), "app").view.product,
            None
        );
        let err = remove_product(&paths, "prod").unwrap_err();
        assert_eq!(err, "unknown product prod");
    }

    #[test]
    fn missing_scan_root_keeps_indexed_projects() {
        let (tmp, paths, _src) = setup();
        overview(&paths, true).unwrap();
        let mut cfg = Config::load(&paths.config_file).unwrap();
        cfg.roots.push(tmp.path().join("gone"));
        cfg.save(&paths.config_file).unwrap();
        let ov = overview(&paths, true).unwrap();
        assert_eq!(ov.projects.len(), 2);
    }

    #[test]
    fn config_watch_ignores_reads_and_sees_writes() {
        let (_tmp, paths, src) = setup();
        let (tx, rx) = std::sync::mpsc::channel();
        let _w = watch(&paths.config_file, move || tx.send(()).unwrap()).unwrap();
        let quiet = std::time::Duration::from_millis(300);
        overview(&paths, false).unwrap();
        assert!(
            rx.recv_timeout(quiet).is_err(),
            "a config read fired a change"
        );
        add_product(&paths, "prod", &src.join("prod")).unwrap();
        assert!(
            rx.recv_timeout(quiet).is_ok(),
            "a config write fired nothing"
        );
    }

    #[test]
    fn detail_returns_context_of_listed_project() {
        let (_tmp, paths, _src) = setup();
        let ov = overview(&paths, true).unwrap();
        let id = find(&ov, "app").view.id.clone();
        assert!(!id.ends_with(".git"));
        let d = detail(&paths, id).unwrap();
        assert_eq!(d.context.name, "app");
        assert_eq!(d.health.worktrees.len(), 1);
        assert_eq!(d.prune.len(), 1);
    }

    #[test]
    fn detail_flags_a_missing_worktree_prunable() {
        let (tmp, paths, src) = setup();
        let app = src.join("prod/app");
        let wt = tmp.path().join("app-wt");
        crate::support::commit_in(&app, "init", None);
        let add = [
            "worktree",
            "add",
            "-q",
            "-b",
            "74-gone",
            wt.to_str().unwrap(),
        ];
        picore_core::process::run("git", &app, &add, TIMEOUT).unwrap();
        fs::remove_dir_all(&wt).unwrap();
        let ov = overview(&paths, true).unwrap();
        let d = detail(&paths, find(&ov, "app").view.id.clone()).unwrap();
        let gone = d
            .prune
            .iter()
            .find(|a| a.branch.as_deref() == Some("74-gone"))
            .unwrap();
        assert_eq!(
            gone.verdict,
            prune::Verdict::Prunable(prune::Reason::Missing)
        );
        let listed = d
            .context
            .worktrees
            .iter()
            .find(|w| w.branch.as_deref() == Some("74-gone"));
        assert_eq!(listed.unwrap().issue, Some(74));
    }
}
