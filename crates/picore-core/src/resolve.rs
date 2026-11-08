use std::env;
use std::path::PathBuf;

use crate::model::{Project, Worktree, project_dir};

#[derive(Debug, PartialEq, Eq)]
pub struct Target<'a> {
    pub project: &'a Project,
    pub worktree: Option<&'a Worktree>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Resolution<'a> {
    One(Target<'a>),
    Many(Vec<&'a Project>),
    None,
}

/// Path (`/`, `~`, `.`), `name@N` (Worktree of issue N), or case-insensitive
/// `name` / `parent/name`.
pub fn resolve<'a>(projects: &'a [Project], query: &str) -> Resolution<'a> {
    if query.starts_with(['/', '~', '.']) {
        return by_path(projects, query);
    }
    if let Some((name, n)) = query.rsplit_once('@')
        && !n.is_empty()
        && n.bytes().all(|b| b.is_ascii_digit())
    {
        let Ok(issue) = n.parse::<u32>() else {
            return Resolution::None;
        };
        return match by_name(projects, name) {
            Resolution::One(t) => {
                let worktree = t.project.worktrees.iter().find(|w| w.issue == Some(issue));
                worktree.map_or(Resolution::None, |w| {
                    Resolution::One(Target {
                        worktree: Some(w),
                        ..t
                    })
                })
            }
            r => r,
        };
    }
    by_name(projects, query)
}

fn by_path<'a>(projects: &'a [Project], query: &str) -> Resolution<'a> {
    let raw = match query.strip_prefix('~') {
        Some(rest) => match env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(rest.trim_start_matches('/')),
            None => return Resolution::None,
        },
        None => PathBuf::from(query),
    };
    let path = raw.canonicalize().unwrap_or(raw);
    projects
        .iter()
        .flat_map(|p| p.worktrees.iter().map(move |w| (p, w)))
        .filter(|(_, w)| path.starts_with(&w.path))
        .max_by_key(|(_, w)| w.path.components().count())
        .map_or(Resolution::None, |(project, w)| {
            Resolution::One(Target {
                project,
                worktree: Some(w),
            })
        })
}

fn by_name<'a>(projects: &'a [Project], query: &str) -> Resolution<'a> {
    let query = query.to_lowercase();
    let (parent, needle) = match query.rsplit_once('/') {
        Some((parent, name)) => (Some(parent), name),
        None => (None, query.as_str()),
    };
    if needle.is_empty() {
        return Resolution::None;
    }
    let candidates: Vec<(&'a Project, String)> = projects
        .iter()
        .filter(|p| parent.is_none_or(|q| parent_name(p) == q))
        .map(|p| (p, p.name.to_lowercase()))
        .collect();
    let stages: [fn(&str, &str) -> bool; 3] = [
        |name, q| name == q,
        |name, q| name.starts_with(q),
        |name, q| name.contains(q),
    ];
    for matches in stages {
        let hits: Vec<&'a Project> = candidates
            .iter()
            .filter(|(_, name)| matches(name, needle))
            .map(|(p, _)| *p)
            .collect();
        match hits.len() {
            0 => continue,
            1 => {
                return Resolution::One(Target {
                    project: hits[0],
                    worktree: None,
                });
            }
            _ => return Resolution::Many(hits),
        }
    }
    Resolution::None
}

/// `parent/name`, the form `resolve` accepts back.
pub fn qualified(p: &Project) -> String {
    format!("{}/{}", parent_name(p), p.name)
}

fn parent_name(p: &Project) -> String {
    let dir = p.work_tree.as_deref().unwrap_or_else(|| project_dir(&p.id));
    dir.parent()
        .and_then(|d| d.file_name())
        .map_or_else(String::new, |n| n.to_string_lossy().to_lowercase())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::model::{ProjectId, project_name};

    fn project(dir: &str) -> Project {
        let id = ProjectId(PathBuf::from(dir).join(".git"));
        Project {
            name: project_name(&id),
            id,
            work_tree: Some(PathBuf::from(dir)),
            branch: None,
            repo: None,
            techs: vec![],
            worktrees: vec![],
            stamp: 0,
        }
    }

    fn names<'a>(hits: &[&'a Project]) -> Vec<&'a str> {
        hits.iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn exact_beats_prefix() {
        let ps = [project("/src/api"), project("/src/apiary")];
        let Resolution::One(Target { project: p, .. }) = resolve(&ps, "api") else {
            panic!("expected One");
        };
        assert_eq!(p.name, "api");
    }

    #[test]
    fn prefix_ambiguous() {
        let ps = [project("/src/app"), project("/src/apple")];
        let Resolution::Many(hits) = resolve(&ps, "ap") else {
            panic!("expected Many");
        };
        assert_eq!(names(&hits), ["app", "apple"]);
    }

    #[test]
    fn qualifier_disambiguates() {
        let ps = [project("/src/client-a/api"), project("/src/client-b/api")];
        let Resolution::One(Target { project: p, .. }) = resolve(&ps, "client-b/api") else {
            panic!("expected One");
        };
        assert_eq!(
            p.work_tree.as_deref(),
            Some(PathBuf::from("/src/client-b/api").as_path())
        );
    }

    #[test]
    fn duplicate_exact_is_many() {
        let ps = [project("/a/api"), project("/b/api")];
        let Resolution::Many(hits) = resolve(&ps, "api") else {
            panic!("expected Many");
        };
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn substring_fallback() {
        let ps = [project("/src/paint-the-dead")];
        let Resolution::One(Target { project: p, .. }) = resolve(&ps, "dead") else {
            panic!("expected One");
        };
        assert_eq!(p.name, "paint-the-dead");
    }

    #[test]
    fn case_insensitive() {
        let ps = [project("/src/Api")];
        assert!(matches!(resolve(&ps, "API"), Resolution::One(_)));
    }

    fn issue_project(dir: &str, branches: &[&str]) -> Project {
        let mut p = project(dir);
        p.worktrees = branches
            .iter()
            .map(|b| Worktree {
                path: PathBuf::from(format!("{dir}/../wt-{b}")),
                branch: Some((*b).to_owned()),
                head: None,
                is_main: false,
                detached: false,
                locked: false,
                last_commit_at: None,
                issue: crate::model::issue_of(b),
            })
            .collect();
        p
    }

    #[test]
    fn name_at_issue_picks_worktree() {
        let ps = [issue_project("/src/api", &["12-a", "80-b"])];
        let Resolution::One(t) = resolve(&ps, "api@80") else {
            panic!("expected One");
        };
        assert_eq!(t.worktree.unwrap().branch.as_deref(), Some("80-b"));
        assert_eq!(resolve(&ps, "api@7"), Resolution::None);
        assert_eq!(resolve(&ps, "api@99999999999"), Resolution::None);
    }

    #[test]
    fn unknown_is_none() {
        let ps = [project("/src/api")];
        assert_eq!(resolve(&ps, "zzz"), Resolution::None);
    }
}
