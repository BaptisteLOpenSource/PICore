use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use picore_core::config::{Config, Paths};
use picore_core::context::{self, ProjectContext, Section};
use picore_core::error::{Error, Result};
use picore_core::health::{ProjectHealth, machine_health, project_health};
use picore_core::model::{Project, ProjectId};
use picore_core::prune::{self, Assessment};
use picore_core::refresh::refresh;
use picore_core::store::Index;
use serde_json::{Value, json};

const HEALTH_TTL: Duration = Duration::from_secs(10);

pub struct Server {
    index: Mutex<Index>,
    cfg: Config,
    health_cache: Mutex<HashMap<ProjectId, (Instant, ProjectHealth)>>,
}

impl Server {
    pub fn open(paths: &Paths) -> Result<Server> {
        Ok(Server {
            index: Mutex::new(Index::open(&paths.index_file)?),
            cfg: Config::load(&paths.config_file)?,
            health_cache: Mutex::default(),
        })
    }

    fn lookup(&self, project: &str) -> Result<Project> {
        let id =
            ProjectId(std::fs::canonicalize(project).map_err(|_| Error::NotFound(project.into()))?);
        let index = self.index.lock().unwrap();
        if index.get(&id)?.is_none() {
            return Err(Error::NotFound(project.into()));
        }
        refresh(&index, &id)?;
        index
            .get(&id)?
            .ok_or_else(|| Error::NotFound(project.into()))
    }

    pub fn list_projects(&self) -> Result<Value> {
        let projects = self.index.lock().unwrap().all()?;
        if projects.is_empty() {
            return Ok(json!({"projects": [], "hint": "run picore index"}));
        }
        let rows: Vec<Value> = projects
            .iter()
            .map(|p| {
                json!({
                    "id": p.id.0, "name": p.name, "techs": p.techs,
                    "branch": p.branch, "worktrees": p.worktrees.len(),
                })
            })
            .collect();
        Ok(json!({"projects": rows}))
    }

    pub fn get_project_context(&self, project: &str) -> Result<ProjectContext> {
        context::context(&self.lookup(project)?)
    }

    pub fn get_project_section(&self, project: &str, file: &str, heading: &str) -> Result<Section> {
        context::section(&self.lookup(project)?, file, heading)
    }

    pub fn list_worktrees(&self, project: &str) -> Result<Vec<Assessment>> {
        let p = self.lookup(project)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        p.worktrees
            .iter()
            .map(|wt| prune::assess(&p, wt, self.cfg.stale_days, now))
            .collect()
    }

    pub fn get_health(&self, project: Option<&str>) -> Result<Value> {
        let project = project.map(|q| self.project_health_cached(q)).transpose()?;
        Ok(json!({"machine": machine_health(), "project": project}))
    }

    fn project_health_cached(&self, query: &str) -> Result<ProjectHealth> {
        let p = self.lookup(query)?;
        let mut cache = self.health_cache.lock().unwrap();
        if let Some((_, h)) = cache.get(&p.id).filter(|(at, _)| at.elapsed() < HEALTH_TTL) {
            return Ok(h.clone());
        }
        let h = project_health(&p, false)?;
        cache.insert(p.id, (Instant::now(), h.clone()));
        Ok(h)
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use picore_core::refresh::scan;
    use tempfile::TempDir;

    use super::*;

    fn server(dir: &TempDir) -> Server {
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            index_file: dir.path().join("index.db"),
        };
        Server::open(&paths).unwrap()
    }

    fn git_project(dir: &TempDir, s: &Server) -> (String, ProjectId) {
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let ok = Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .arg(&repo)
            .status()
            .unwrap();
        assert!(ok.success());
        let id = ProjectId(std::fs::canonicalize(&repo).unwrap());
        s.index.lock().unwrap().upsert(&scan(&id).unwrap()).unwrap();
        (id.0.display().to_string(), id)
    }

    #[test]
    fn unknown_project_is_not_found() {
        let dir = TempDir::new().unwrap();
        let s = server(&dir);
        assert!(matches!(s.lookup("/tmp/nope"), Err(Error::NotFound(_))));
    }

    #[test]
    fn health_cached_ten_seconds() {
        let dir = TempDir::new().unwrap();
        let s = server(&dir);
        let (path, id) = git_project(&dir, &s);
        s.get_health(Some(&path)).unwrap();
        let first = s.health_cache.lock().unwrap()[&id].0;
        s.get_health(Some(&path)).unwrap();
        assert_eq!(s.health_cache.lock().unwrap()[&id].0, first);
    }
}
