use std::fs;
use std::io::{self, Read};
use std::path::Path;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::model::{Project, Worktree};
use crate::view::ProjectView;

pub const WHITELIST: [&str; 3] = ["README.md", "CLAUDE.md", "AGENTS.md"];

const INTRO_MAX: usize = 2048;
const HEADING_MAX: usize = 512;
const OUTLINE_MAX: usize = 100;
const SECTION_MAX: usize = 16384;
const FILE_MAX: u64 = 1 << 20;

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
    pub product: Option<String>,
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

const TAGS: &[&str] = &[
    "a", "b", "i", "u", "p", "br", "hr", "img", "div", "span", "em", "strong", "code", "pre",
    "details", "summary", "picture", "source", "sup", "sub", "center", "table", "tr", "td", "th",
    "ul", "ol", "li", "kbd", "svg", "h1", "h2", "h3", "h4", "h5", "h6",
];

// Length of a known HTML tag at the start of `s`, 0 if none. Stops at the
// next `<` or newline, so unclosed input stays linear.
fn tag_len(s: &str) -> usize {
    let body = &s[1..];
    let name = body.strip_prefix('/').unwrap_or(body);
    let n = name
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(name.len());
    if !TAGS.contains(&&name[..n]) {
        return 0;
    }
    match body.find(['>', '<', '\n']) {
        Some(e) if body.as_bytes()[e] == b'>' => e + 2,
        _ => 0,
    }
}

// Drops HTML comments, tags and markdown images; link text stays.
pub fn strip_markup(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    let mut no_image = false;
    while let Some(i) = rest.find(['<', '!']) {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let skip = if let Some(r) = rest.strip_prefix("<!--") {
            r.find("-->").map_or(rest.len(), |e| 4 + e + 3)
        } else if rest.starts_with('<') {
            tag_len(rest)
        } else if rest.starts_with("![") && !no_image {
            let end = rest
                .find("](")
                .and_then(|a| rest[a..].find(')').map(|b| a + b + 1));
            no_image = end.is_none();
            end.unwrap_or(0)
        } else {
            0
        };
        if skip == 0 {
            out.push_str(&rest[..1]);
        }
        rest = &rest[skip.max(1)..];
    }
    out.push_str(rest);
    let mut clean = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some(i) = rest.find("[](") {
        clean.push_str(&rest[..i]);
        rest = &rest[i..];
        match rest.find(')') {
            Some(e) => rest = &rest[e + 1..],
            None => break,
        }
    }
    clean.push_str(rest);
    let mut lines: Vec<&str> = Vec::new();
    for l in clean.lines().map(str::trim_end) {
        if !(l.is_empty() && lines.last().is_none_or(|p| p.is_empty())) {
            lines.push(l);
        }
    }
    lines.join("\n").trim().to_owned()
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
                    text: strip_markup(cut(text.to_owned(), HEADING_MAX).0.as_str()),
                });
            } else {
                headings_truncated = true;
            }
        }
    }
    let (intro, intro_truncated) = cut(intro, INTRO_MAX * 4);
    let (intro, cut_more) = cut(strip_markup(&intro), INTRO_MAX);
    let intro_truncated = intro_truncated || cut_more;
    (intro, intro_truncated, headings, headings_truncated)
}

pub fn section_of(md: &str, heading: &str) -> Option<Section> {
    let mut body = String::new();
    let mut level = None;
    for line in lines(md) {
        match (level, line.heading) {
            (None, Some((lvl, text))) if text == heading || strip_markup(text) == heading => {
                level = Some(lvl)
            }
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
    let path = root.join(file);
    // Symlinks (even in-root ones like README.md -> .env) and FIFOs are refused.
    match fs::symlink_metadata(&path) {
        Ok(m) if m.is_file() => {
            let mut buf = Vec::new();
            fs::File::open(path)?.take(FILE_MAX).read_to_end(&mut buf)?;
            Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
        }
        Ok(_) => Err(Error::Config("not a regular file".to_owned())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn context(p: &Project, view: &ProjectView) -> Result<ProjectContext> {
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
        name: view.name.clone(),
        product: view.product.clone(),
        branch: p.branch.clone(),
        techs: p.techs.clone(),
        worktrees: p.worktrees.clone(),
        files,
    })
}

pub fn section(p: &Project, file: &str, heading: &str) -> Result<Section> {
    if !WHITELIST.contains(&file) {
        return Err(Error::Config("not whitelisted".to_owned()));
    }
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
    fn strip_markup_removes_html_images_and_badges() {
        let md = "<div align=\"center\">\n<!-- hidden\nbadge -->\n[![CI](https://x/ci.svg)](https://x)\n![logo](l.png)\n</div>\n\n\n# PICore <img src=\"a.png\"> tool\nSee [docs](https://d) a < b! ok";
        assert_eq!(
            strip_markup(md),
            "# PICore  tool\nSee [docs](https://d) a < b! ok"
        );
    }

    #[test]
    fn strip_markup_is_linear_on_unclosed_input() {
        let t = std::time::Instant::now();
        for unit in ["<a", "[](", "![", "![](", "<a<a>"] {
            strip_markup(&unit.repeat(100_000));
        }
        assert!(t.elapsed().as_secs() < 2);
    }

    #[test]
    fn strip_markup_keeps_generics() {
        assert_eq!(
            strip_markup("Using Vec<String> and <T>"),
            "Using Vec<String> and <T>"
        );
        let (_, _, h, _) = outline("# Using Vec<String>\n");
        assert_eq!(h[0].text, "Using Vec<String>");
    }

    #[test]
    fn outline_and_section_use_stripped_text() {
        let md = "# A <b>x</b>\n<p>hi</p>\n## Use ![b](u.png) it\nbody";
        let (intro, _, headings, _) = outline(md);
        assert!(!intro.contains('<'));
        assert_eq!(headings[0].text, "A x");
        assert_eq!(headings[1].text, "Use  it");
        assert_eq!(
            section_of(md, &headings[1].text).unwrap().text.trim(),
            "body"
        );
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
