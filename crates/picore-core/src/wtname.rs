use std::path::Path;

use serde::Serialize;

use crate::error::Result;
use crate::gh;

const MAX_SLUG: usize = 40;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct WorktreeName {
    pub branch: String,
    pub path: String,
}

fn fold(c: char) -> &'static str {
    match c {
        'à' | 'â' | 'ä' => "a",
        'é' | 'è' | 'ê' | 'ë' => "e",
        'î' | 'ï' => "i",
        'ô' | 'ö' => "o",
        'ù' | 'û' | 'ü' => "u",
        'ç' => "c",
        'œ' => "oe",
        'æ' => "ae",
        _ => "",
    }
}

pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.to_lowercase().chars() {
        let folded = fold(c);
        let mut buf = [0u8; 4];
        let s = if c.is_ascii_alphanumeric() {
            c.encode_utf8(&mut buf)
        } else {
            folded
        };
        if s.is_empty() {
            if !out.ends_with('-') {
                out.push('-');
            }
        } else {
            out.push_str(s);
        }
    }
    let out = out.trim_matches('-');
    if out.len() <= MAX_SLUG {
        return out.into();
    }
    let cut = &out[..MAX_SLUG];
    let next_is_dash = out.as_bytes()[MAX_SLUG] == b'-';
    let cut = if next_is_dash {
        cut
    } else {
        cut.rsplit_once('-').map_or(cut, |(head, _)| head)
    };
    cut.trim_matches('-').into()
}

pub fn worktree_name(project_dir: &Path, issue: u32, title: &str) -> WorktreeName {
    let branch = format!("{issue}-{}", slug(title));
    let name = project_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let path = project_dir
        .parent()
        .unwrap_or(project_dir)
        .join(format!("{name}.worktrees"))
        .join(&branch);
    WorktreeName {
        branch,
        path: path.display().to_string(),
    }
}

/// Fetches the issue title with gh run in the Project, so gh picks its repo.
pub fn for_issue(project_dir: &Path, issue: u32) -> Result<WorktreeName> {
    let title = gh::issue_title(project_dir, issue)?;
    Ok(worktree_name(project_dir, issue, &title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_table() {
        for (title, want) in [
            ("Fix the Bug", "fix-the-bug"),
            (
                "Créer l'élève: œuvre & æon à côté",
                "creer-l-eleve-oeuvre-aeon-a-cote",
            ),
            ("ça, où ? ünï", "ca-ou-uni"),
            ("  --Hello!!  ", "hello"),
            ("rocket 🚀 launch", "rocket-launch"),
            ("日本語 title", "title"),
            ("🚀", ""),
            ("a".repeat(50).as_str(), &"a".repeat(40)),
            (
                "add a very long worktree naming feature now",
                "add-a-very-long-worktree-naming-feature",
            ),
            (
                "exactly-forty-chars-long-slug-ok-1234567 y",
                "exactly-forty-chars-long-slug-ok-1234567",
            ),
        ] {
            assert_eq!(slug(title), want, "{title}");
        }
    }

    #[test]
    fn layout_beside_main_project() {
        let n = worktree_name(Path::new("/g/Config/PICore"), 82, "Wt Name!");
        assert_eq!(n.branch, "82-wt-name");
        assert_eq!(n.path, "/g/Config/PICore.worktrees/82-wt-name");
    }
}
