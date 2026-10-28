use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::model::{Project, Worktree};

pub const WHITELIST: [&str; 3] = ["README.md", "CLAUDE.md", "AGENTS.md"];

const INTRO_MAX: usize = 2048;
const OUTLINE_MAX: usize = 100;
const SECTION_MAX: usize = 16384;

#[derive(Debug, Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct FileOutline {
    pub file: String,
    pub intro: String,
    pub intro_truncated: bool,
    pub headings: Vec<Heading>,
    pub headings_truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct ProjectContext {
    pub name: String,
    pub branch: Option<String>,
    pub techs: Vec<String>,
    pub worktrees: Vec<Worktree>,
    pub files: Vec<FileOutline>,
}

#[derive(Debug, Serialize)]
pub struct Section {
    pub text: String,
    pub truncated: bool,
}

struct Line<'a> {
    raw: &'a str,
    heading: Option<(u8, &'a str)>,
}

// Fenced blocks are classified as plain text so `# comment` inside code never reads as a heading.
fn lines(md: &str) -> Vec<Line<'_>> {
    let mut fence: Option<char> = None;
    md.lines()
        .map(|raw| {
            let t = raw.trim_start();
            if let Some(c) = fence {
                if t.starts_with([c, c, c]) && t.trim_end().chars().all(|x| x == c) {
                    fence = None;
                }
                return Line { raw, heading: None };
            }
            if t.starts_with("```") {
                fence = Some('`');
                return Line { raw, heading: None };
            }
            if t.starts_with("~~~") {
                fence = Some('~');
                return Line { raw, heading: None };
            }
            Line {
                raw,
                heading: atx(raw),
            }
        })
        .collect()
}

fn atx(line: &str) -> Option<(u8, &str)> {
    let t = line.trim_start();
    if line.len() - t.len() > 3 {
        return None;
    }
    let level = t.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &t[level..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    Some((level as u8, heading_text(rest)))
}

// Closing `##` only counts when preceded by a space, so `C#` keeps its `#`.
fn heading_text(rest: &str) -> &str {
    let t = rest.trim_end();
    let stripped = t.trim_end_matches('#');
    if stripped.len() == t.len() || stripped.is_empty() || stripped.ends_with([' ', '\t']) {
        stripped.trim()
    } else {
        t.trim()
    }
}

fn cut(s: String, max: usize) -> (String, bool) {
    if s.len() <= max {
        return (s, false);
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut s = s;
    s.truncate(end);
    (s, true)
}

pub fn outline(md: &str) -> (String, bool, Vec<Heading>, bool) {
    let mut intro = String::new();
    let mut in_intro = true;
    let mut headings = Vec::new();
    let mut headings_truncated = false;
    for line in lines(md) {
        if matches!(line.heading, Some((2, _))) {
            in_intro = false;
        }
        if in_intro {
            intro.push_str(line.raw);
            intro.push('\n');
        }
        if let Some((level, text)) = line.heading {
            if headings.len() < OUTLINE_MAX {
                headings.push(Heading {
                    level,
                    text: text.to_owned(),
                });
            } else {
                headings_truncated = true;
            }
        }
    }
    let (intro, intro_truncated) = cut(intro, INTRO_MAX);
    (intro, intro_truncated, headings, headings_truncated)
}

pub fn section_of(md: &str, heading: &str) -> Option<Section> {
    let mut body = String::new();
    let mut level = None;
    for line in lines(md) {
        match (level, line.heading) {
            (None, Some((lvl, text))) if text == heading => level = Some(lvl),
            (Some(open), Some((lvl, _))) if lvl <= open => break,
            (Some(_), _) => {
                body.push_str(line.raw);
                body.push('\n');
            }
            _ => {}
        }
    }
    level?;
    let (text, truncated) = cut(body, SECTION_MAX);
    Some(Section { text, truncated })
}

pub fn safe_read(root: &Path, file: &str) -> Result<Option<String>> {
    if !WHITELIST.contains(&file) {
        return Err(Error::Config("not whitelisted".to_owned()));
    }
    let root = root.canonicalize()?;
    // Canonicalize first: a symlink named README.md must not escape the root.
    match root.join(file).canonicalize() {
        Ok(path) if path.starts_with(&root) => Ok(Some(fs::read_to_string(path)?)),
        Ok(_) => Err(Error::Config("outside project".to_owned())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn context(p: &Project) -> Result<ProjectContext> {
    let mut files = Vec::new();
    if let Some(root) = &p.work_tree {
        for name in WHITELIST {
            if let Some(md) = safe_read(root, name)? {
                let (intro, intro_truncated, headings, headings_truncated) = outline(&md);
                files.push(FileOutline {
                    file: name.to_owned(),
                    intro,
                    intro_truncated,
                    headings,
                    headings_truncated,
                });
            }
        }
    }
    Ok(ProjectContext {
        name: p.name.clone(),
        branch: p.branch.clone(),
        techs: p.techs.clone(),
        worktrees: p.worktrees.clone(),
        files,
    })
}

pub fn section(p: &Project, file: &str, heading: &str) -> Result<Section> {
    let root = p
        .work_tree
        .as_deref()
        .ok_or_else(|| Error::NotFound(format!("work tree of {}", p.name)))?;
    let md = safe_read(root, file)?.ok_or_else(|| Error::NotFound(file.to_owned()))?;
    section_of(&md, heading).ok_or_else(|| Error::NotFound(format!("{file}#{heading}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_skips_fenced_headings() {
        let (intro, _, headings, _) = outline("# A\nintro\n```\n## fake\n```\n## B");
        let got: Vec<(u8, &str)> = headings
            .iter()
            .map(|h| (h.level, h.text.as_str()))
            .collect();
        assert_eq!(got, [(1, "A"), (2, "B")]);
        assert!(intro.contains("intro"));
    }

    #[test]
    fn intro_truncated_at_2048() {
        let md = format!("{}\n## B", "x".repeat(5000));
        let (intro, truncated, _, _) = outline(&md);
        assert!(intro.len() <= INTRO_MAX);
        assert!(truncated);
    }

    #[test]
    fn section_stops_at_same_level() {
        let s = section_of("## Install\nx\n### Sub\ny\n## Usage", "Install").unwrap();
        assert!(s.text.contains('y'));
        assert!(!s.text.contains("Usage"));
    }

    #[test]
    fn section_truncated_at_16k() {
        let md = format!("## Big\n{}", "y".repeat(20000));
        let s = section_of(&md, "Big").unwrap();
        assert!(s.text.len() <= SECTION_MAX);
        assert!(s.truncated);
    }
}
