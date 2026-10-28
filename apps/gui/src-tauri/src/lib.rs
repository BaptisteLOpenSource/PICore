use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use picore_core::config::{Config, Paths};
use picore_core::context::{self, ProjectContext};
use picore_core::health::{self, Check, ProjectHealth};
use picore_core::model::{Project, ProjectId, project_dir};
use picore_core::refresh;
use picore_core::store::Index;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize)]
pub struct Listed {
    #[serde(flatten)]
    pub project: Project,
    pub product: Option<String>,
    pub health: Option<ProjectHealth>,
}

#[derive(Serialize)]
pub struct Overview {
    pub projects: Vec<Listed>,
    pub products: BTreeMap<String, PathBuf>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct Detail {
    pub context: ProjectContext,
    pub health: ProjectHealth,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub fn overview(paths: &Paths, discover: bool) -> Result<Overview, String> {
    let mut warnings = Vec::new();
    let (cfg, loaded) = match Config::load(&paths.config_file) {
        Ok(cfg) => (cfg, true),
        Err(e) => {
            warnings.push(format!("{e}: products hidden"));
            (Config::default(), false)
        }
    };
    let index = Index::open(&paths.index_file).map_err(err)?;
    if discover && loaded {
        let report = refresh::index_all(&index, &cfg).map_err(err)?;
        warnings.extend(
            report
                .failed
                .iter()
                .map(|f| format!("{}: {}", f.id.0.display(), f.error)),
        );
    }
    let (projects, failed) = refresh::refresh_all(&index).map_err(err)?;
    warnings.extend(
        failed
            .iter()
            .map(|(id, e)| format!("{}: {e}", id.0.display())),
    );
    if projects.is_empty() {
        warnings.push("no projects indexed: add a scan root".into());
    }
    let projects = projects
        .into_iter()
        .map(|project| {
            let product = cfg.product_of(project_dir(&project.id)).map(str::to_owned);
            let health = health::project_health(&project, false).ok();
            Listed {
                project,
                product,
                health,
            }
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
        warnings,
    })
}

pub fn detail(paths: &Paths, id: PathBuf) -> Result<Detail, String> {
    let index = Index::open(&paths.index_file).map_err(err)?;
    let project = index
        .get(&ProjectId(id))
        .map_err(err)?
        .ok_or("unknown project")?;
    Ok(Detail {
        context: context::context(&project).map_err(err)?,
        health: health::project_health(&project, false).map_err(err)?,
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
}

fn watch_config(app: &AppHandle, config_file: &Path) -> Result<RecommendedWatcher, String> {
    let dir = config_file.parent().ok_or("config path has no parent")?;
    fs::create_dir_all(dir).map_err(err)?;
    let target = config_file.to_path_buf();
    let handle = app.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if matches!(event, Ok(e) if e.paths.contains(&target)) {
            let _ = handle.emit("config-changed", ());
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
        ])
        .run(tauri::generate_context!())
        .expect("tauri app");
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use picore_core::config::{Config, Paths};
    use picore_core::model::ProjectId;
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
        ov.projects.iter().find(|l| l.project.name == name).unwrap()
    }

    #[test]
    fn overview_lists_projects_with_products() {
        let (_tmp, paths, src) = setup();
        add_product(&paths, "prod", &src.join("prod")).unwrap();
        let ov = overview(&paths, true).unwrap();
        assert_eq!(ov.projects.len(), 2);
        assert_eq!(find(&ov, "app").product.as_deref(), Some("prod"));
        assert_eq!(find(&ov, "solo").product, None);
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
    fn remove_product_then_unknown() {
        let (_tmp, paths, src) = setup();
        add_product(&paths, "prod", &src.join("prod")).unwrap();
        overview(&paths, true).unwrap();
        remove_product(&paths, "prod").unwrap();
        assert_eq!(find(&overview(&paths, false).unwrap(), "app").product, None);
        let err = remove_product(&paths, "prod").unwrap_err();
        assert_eq!(err, "unknown product prod");
    }

    #[test]
    fn detail_returns_context_of_listed_project() {
        let (_tmp, paths, _src) = setup();
        let ov = overview(&paths, true).unwrap();
        let ProjectId(id) = find(&ov, "app").project.id.clone();
        let d = detail(&paths, id).unwrap();
        assert_eq!(d.context.name, "app");
    }
}
