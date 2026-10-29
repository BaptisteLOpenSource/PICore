use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{Error, Result};
use crate::model::{Project, ProjectId, Worktree};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS project(id TEXT PRIMARY KEY, name TEXT NOT NULL, work_tree TEXT, branch TEXT, techs TEXT NOT NULL, stamp INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS worktree(path TEXT NOT NULL, project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE, branch TEXT, head TEXT, is_main INTEGER NOT NULL, detached INTEGER NOT NULL, locked INTEGER NOT NULL, last_commit_at INTEGER, PRIMARY KEY(project_id, path));
";

const VERSION: i64 = 2;

pub struct Index {
    conn: Connection,
}

impl Index {
    pub fn open(path: &Path) -> Result<Index> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(Duration::from_secs(5))?;
        // Per connection: without it `ON DELETE CASCADE` is a no-op.
        conn.pragma_update(None, "foreign_keys", true)?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version != VERSION {
            // The Index is a rebuildable cache: drop instead of migrating.
            conn.execute_batch("DROP TABLE IF EXISTS worktree; DROP TABLE IF EXISTS project;")?;
        }
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", VERSION)?;
        Ok(Index { conn })
    }

    pub fn upsert(&self, p: &Project) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let id = text(&p.id.0)?;
        // Techs are plain identifiers, so a comma-joined column avoids a JSON dependency.
        tx.execute(
            "INSERT INTO project(id, name, work_tree, branch, techs, stamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, work_tree = excluded.work_tree,
             branch = excluded.branch, techs = excluded.techs, stamp = excluded.stamp",
            params![
                id,
                p.name,
                p.work_tree.as_deref().map(text).transpose()?,
                p.branch,
                p.techs.join(","),
                p.stamp
            ],
        )?;
        tx.execute("DELETE FROM worktree WHERE project_id = ?1", [id])?;
        for w in &p.worktrees {
            tx.execute(
                "INSERT INTO worktree(path, project_id, branch, head, is_main, detached, locked, last_commit_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    text(&w.path)?,
                    id,
                    w.branch,
                    w.head,
                    w.is_main,
                    w.detached,
                    w.locked,
                    w.last_commit_at
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get(&self, id: &ProjectId) -> Result<Option<Project>> {
        let row = self
            .conn
            .query_row(
                "SELECT name, work_tree, branch, techs, stamp FROM project WHERE id = ?1",
                [text(&id.0)?],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((name, work_tree, branch, techs, stamp)) = row else {
            return Ok(None);
        };
        Ok(Some(Project {
            id: id.clone(),
            name,
            work_tree: work_tree.map(PathBuf::from),
            branch,
            techs: techs
                .split(',')
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect(),
            worktrees: self.worktrees_of(id)?,
            stamp,
        }))
    }

    pub fn find(&self, path: &Path) -> Result<Option<Project>> {
        let Ok(path) = path.canonicalize() else {
            return Ok(None);
        };
        let id: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM project WHERE id = ?1 OR work_tree = ?1",
                [text(&path)?],
                |r| r.get(0),
            )
            .optional()?;
        id.map_or(Ok(None), |id| self.get(&ProjectId(PathBuf::from(id))))
    }

    pub fn all(&self) -> Result<Vec<Project>> {
        let ids: Vec<String> = self
            .conn
            .prepare("SELECT id FROM project ORDER BY name, id")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        ids.into_iter()
            .filter_map(|id| self.get(&ProjectId(PathBuf::from(id))).transpose())
            .collect()
    }

    pub fn remove(&self, id: &ProjectId) -> Result<()> {
        self.conn
            .execute("DELETE FROM project WHERE id = ?1", [text(&id.0)?])?;
        Ok(())
    }

    pub fn retain(&self, keep: &[ProjectId]) -> Result<usize> {
        let ids: Vec<String> = self
            .conn
            .prepare("SELECT id FROM project")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let stale = ids
            .iter()
            .filter(|id| !keep.iter().any(|k| k.0.as_path() == Path::new(id)));
        let mut removed = 0;
        for id in stale {
            self.conn
                .execute("DELETE FROM project WHERE id = ?1", [id])?;
            removed += 1;
        }
        Ok(removed)
    }

    fn worktrees_of(&self, id: &ProjectId) -> Result<Vec<Worktree>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, branch, head, is_main, detached, locked, last_commit_at
             FROM worktree WHERE project_id = ?1 ORDER BY is_main DESC, path",
        )?;
        let rows = stmt.query_map([text(&id.0)?], |r| {
            Ok(Worktree {
                path: PathBuf::from(r.get::<_, String>(0)?),
                branch: r.get(1)?,
                head: r.get(2)?,
                is_main: r.get(3)?,
                detached: r.get(4)?,
                locked: r.get(5)?,
                last_commit_at: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

fn text(p: &Path) -> Result<&str> {
    p.to_str().ok_or_else(|| {
        Error::from(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("non-UTF-8 path: {}", p.display()),
        ))
    })
}
