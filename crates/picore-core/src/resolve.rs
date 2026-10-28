use crate::model::{Project, project_dir};

#[derive(Debug, PartialEq, Eq)]
pub enum Resolution<'a> {
    One(&'a Project),
    Many(Vec<&'a Project>),
    None,
}

/// Case-insensitive. `parent/name` restricts matches to Projects whose parent dir is `parent`.
pub fn resolve<'a>(projects: &'a [Project], query: &str) -> Resolution<'a> {
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
            1 => return Resolution::One(hits[0]),
            _ => return Resolution::Many(hits),
        }
    }
    Resolution::None
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
        let Resolution::One(p) = resolve(&ps, "api") else {
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
        let Resolution::One(p) = resolve(&ps, "client-b/api") else {
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
        let Resolution::One(p) = resolve(&ps, "dead") else {
            panic!("expected One");
        };
        assert_eq!(p.name, "paint-the-dead");
    }

    #[test]
    fn case_insensitive() {
        let ps = [project("/src/Api")];
        assert!(matches!(resolve(&ps, "API"), Resolution::One(_)));
    }

    #[test]
    fn unknown_is_none() {
        let ps = [project("/src/api")];
        assert_eq!(resolve(&ps, "zzz"), Resolution::None);
    }
}
