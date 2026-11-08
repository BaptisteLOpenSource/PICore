use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::CommandFactory;
use clap_complete::Shell;
use picore_core::config::{Config, Paths, editor_command};
use picore_core::discovery::suggest_roots;
use picore_core::error::{Error, Result};
use picore_core::health::{Check, ProjectHealth, machine_health, project_health};
use picore_core::info::{self, Info};
use picore_core::model::{Project, project_dir};
use picore_core::prune::{self, Assessment, Blocker, Verdict};
use picore_core::refresh::{index_all, refresh_all};
use picore_core::resolve::{Resolution, Target, qualified, resolve};
use picore_core::store::Index;
use picore_core::wtname;
use serde::Serialize;

use crate::output::{emit, fail, mascot};
use crate::{Cli, Cmd, InitShell, RootCmd};

const GUI_BIN: &str = "picore-gui";

pub fn run(json: bool, quiet: bool, cmd: &Cmd) -> ExitCode {
    let result = match cmd {
        Cmd::Root { action } => root(json, action),
        Cmd::Index => index(json, quiet),
        Cmd::List => list(json),
        Cmd::Gui => gui(json),
        Cmd::Open { name } => open(json, name),
        Cmd::Path { query } => path_cmd(json, query),
        Cmd::Info { query } => info_cmd(json, query),
        Cmd::WtName { issue, project } => wt_name(json, *issue, project.as_deref()),
        Cmd::Init { shell } => init(*shell),
        Cmd::Status { name, remote } => status(json, name.as_deref(), *remote),
        Cmd::Worktrees { name } => worktrees(json, name.as_deref()),
        Cmd::Prune { target, yes, force } => prune_cmd(json, target.as_deref(), *yes, *force),
        Cmd::Completions { shell } => completions(*shell),
        Cmd::Man => man(),
        Cmd::Mcp => unreachable!("main.rs dispatches mcp"),
    };
    result.unwrap_or_else(|e| match e {
        Error::Ambiguous(candidates) => fail(json, "ambiguous", "ambiguous name", &candidates),
        e => fail(json, code_of(&e), &e.to_string(), &[]),
    })
}

fn completions(shell: Shell) -> Result<ExitCode> {
    clap_complete::generate(shell, &mut Cli::command(), "picore", &mut io::stdout());
    Ok(ExitCode::SUCCESS)
}

fn man() -> Result<ExitCode> {
    clap_mangen::Man::new(Cli::command()).render(&mut io::stdout())?;
    Ok(ExitCode::SUCCESS)
}

fn code_of(e: &Error) -> &'static str {
    match e {
        Error::Proc(_) => "git",
        Error::Gh(_) => "gh",
        Error::Config(_) => "config",
        Error::NotFound(_) => "not_found",
        Error::Ambiguous(_) => "ambiguous",
        Error::Store(_) | Error::Io(_) => "io",
    }
}

fn root(json: bool, action: &RootCmd) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    let mut cfg = Config::load(&paths.config_file)?;
    match action {
        RootCmd::Add { path } => {
            cfg.add_root(path)?;
        }
        RootCmd::Remove { path } => {
            if !cfg.remove_root(path) {
                return Err(Error::NotFound(path.display().to_string()));
            }
        }
        RootCmd::List => {}
    }
    if !matches!(action, RootCmd::List) {
        cfg.save(&paths.config_file)?;
    }
    emit(json, &cfg.roots, |roots| {
        for r in roots {
            println!("{}", r.display());
        }
    });
    Ok(ExitCode::SUCCESS)
}

fn index(json: bool, quiet: bool) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    let mut cfg = Config::load(&paths.config_file)?;
    if cfg.roots.is_empty() {
        let home = env::var_os("HOME").map(PathBuf::from);
        let candidates = home.as_deref().map(suggest_roots).unwrap_or_default();
        let names: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
        if candidates.is_empty() || json || !io::stdin().is_terminal() {
            return Ok(fail(
                json,
                "no_roots",
                "no scan roots: run `picore root add <path>`",
                &names,
            ));
        }
        for n in &names {
            eprintln!("  {n}");
        }
        if !confirm("Add these scan roots? [Y/n] ")? {
            return Ok(fail(json, "no_roots", "no scan roots configured", &[]));
        }
        for c in &candidates {
            cfg.add_root(c)?;
        }
        cfg.save(&paths.config_file)?;
    }
    let index = Index::open(&paths.index_file)?;
    let report = index_all(&index, &cfg)?;
    for f in &report.failed {
        eprintln!("warn: {}: {}", f.id.0.display(), f.error);
    }
    emit(json, &report, |r| {
        println!(
            "indexed {} projects: {} updated, {} removed",
            r.found, r.updated, r.removed
        );
    });
    mascot(json, quiet, "indexed");
    Ok(ExitCode::SUCCESS)
}

fn confirm(prompt: &str) -> io::Result<bool> {
    eprint!("{prompt}");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "" | "y" | "yes"
    ))
}

fn refreshed(paths: &Paths) -> Result<Vec<Project>> {
    let (projects, failed) = refresh_all(&Index::open(&paths.index_file)?)?;
    for (id, e) in failed {
        eprintln!("warn: {}: {e}", id.0.display());
    }
    Ok(projects)
}

#[derive(Serialize)]
struct Listed<'a> {
    #[serde(flatten)]
    project: &'a Project,
    product: Option<&'a str>,
}

fn list(json: bool) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    // bad config must not hide the list: drop products, keep going
    let cfg = Config::load(&paths.config_file).unwrap_or_else(|e| {
        eprintln!("warn: {e}: products hidden");
        Config::default()
    });
    let projects = refreshed(&paths)?;
    if projects.is_empty() {
        eprintln!("no projects indexed: run `picore index`");
    }
    let listed: Vec<Listed> = projects
        .iter()
        .map(|p| Listed {
            project: p,
            product: cfg.product_of(project_dir(&p.id)),
        })
        .collect();
    emit(json, &listed, |l| print_table(l));
    Ok(ExitCode::SUCCESS)
}

/// Products by name, members by name, then ungrouped Projects by name.
fn grouped<'a, 'b>(listed: &'b [Listed<'a>]) -> Vec<(Option<&'a str>, Vec<&'b Listed<'a>>)> {
    let mut by_product: BTreeMap<Option<&str>, Vec<&Listed>> = BTreeMap::new();
    for l in listed {
        by_product.entry(l.product).or_default().push(l);
    }
    let mut groups: Vec<_> = by_product.into_iter().collect();
    // BTreeMap puts None first: ungrouped goes last
    let shift = usize::from(groups.first().is_some_and(|g| g.0.is_none()));
    groups.rotate_left(shift);
    for (_, members) in &mut groups {
        members.sort_by(|a, b| a.project.name.cmp(&b.project.name));
    }
    groups
}

fn print_table(listed: &[Listed]) {
    if listed.is_empty() {
        return;
    }
    let header = ["NAME", "PRODUCT", "BRANCH", "WT", "TECHS", "REPO", "PATH"].map(String::from);
    let mut lines: Vec<(Option<&str>, [String; 7])> = vec![(None, header)];
    for (product, members) in grouped(listed) {
        if let Some(name) = product {
            lines.push((Some(name), Default::default()));
        }
        for l in members {
            let p = l.project;
            let indent = if product.is_some() { "  " } else { "" };
            lines.push((
                None,
                [
                    format!("{indent}{}", p.name),
                    l.product.unwrap_or("-").into(),
                    p.branch.clone().unwrap_or_else(|| "-".into()),
                    p.worktrees.len().to_string(),
                    if p.techs.is_empty() {
                        "-".into()
                    } else {
                        p.techs.join(",")
                    },
                    p.repo.clone().unwrap_or_else(|| "-".into()),
                    p.work_tree
                        .as_deref()
                        .unwrap_or(&p.id.0)
                        .display()
                        .to_string(),
                ],
            ));
        }
    }
    let widths: [usize; 7] = std::array::from_fn(|c| {
        lines
            .iter()
            .filter(|(h, _)| h.is_none())
            .map(|(_, r)| r[c].chars().count())
            .max()
            .unwrap_or(0)
    });
    for (header, row) in lines {
        if let Some(name) = header {
            println!("{name}");
            continue;
        }
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(c, cell)| {
                if c == 6 {
                    cell.clone()
                } else {
                    format!("{cell:<w$}", w = widths[c])
                }
            })
            .collect();
        println!("{}", cells.join("  "));
    }
}

#[derive(Serialize)]
struct OpenReport<'a> {
    path: &'a Path,
    editor: &'a [String],
}

/// Like `one`, but keeps the Worktree and lets a TTY pick among candidates.
fn target<'a>(json: bool, projects: &'a [Project], query: &str) -> Result<Target<'a>> {
    match resolve(projects, query) {
        Resolution::Many(hits) if !json && io::stdin().is_terminal() => Ok(Target {
            project: pick(hits)?,
            worktree: None,
        }),
        r => r.into_target(query),
    }
}

/// Directory a query points at; Err = refusal already reported.
fn target_dir(
    json: bool,
    projects: &[Project],
    name: &str,
) -> Result<std::result::Result<PathBuf, ExitCode>> {
    let t = target(json, projects, name)?;
    Ok(t.dir().map(Path::to_path_buf).ok_or_else(|| {
        fail(
            json,
            "refused",
            &format!("{}: no worktree", t.project.name),
            &[],
        )
    }))
}

#[derive(Serialize)]
struct PathReport<'a> {
    path: &'a Path,
}

fn path_cmd(json: bool, name: &str) -> Result<ExitCode> {
    let projects = refreshed(&Paths::from_env()?)?;
    let path = match target_dir(json, &projects, name)? {
        Ok(p) => p,
        Err(code) => return Ok(code),
    };
    emit(json, &PathReport { path: &path }, |r| {
        println!("{}", r.path.display())
    });
    Ok(ExitCode::SUCCESS)
}

fn info_cmd(json: bool, query: &str) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    // bad config must not hide the info: drop products, keep going
    let cfg = Config::load(&paths.config_file).unwrap_or_else(|e| {
        eprintln!("warn: {e}: products hidden");
        Config::default()
    });
    let projects = refreshed(&paths)?;
    let t = target(json, &projects, query)?;
    let report = info::info(&t, cfg.product_of(project_dir(&t.project.id)))?;
    emit(json, &report, print_info);
    Ok(ExitCode::SUCCESS)
}

fn print_info(i: &Info) {
    let dash = |s: Option<&str>| s.unwrap_or("-").to_owned();
    let mut rows = vec![
        ("name", i.name.clone()),
        ("product", dash(i.product.as_deref())),
        ("path", i.path.display().to_string()),
        ("repo", dash(i.repo.as_deref())),
        ("branch", dash(i.branch.as_deref())),
        (
            "tags",
            if i.tags.is_empty() {
                "-".into()
            } else {
                i.tags.join(", ")
            },
        ),
    ];
    if let Some(g) = &i.github {
        rows.extend([
            ("description", g.description.clone()),
            ("url", g.url.clone()),
            ("visibility", g.visibility.clone()),
            ("topics", g.topics.join(", ")),
            ("release", dash(g.latest_release.as_deref())),
            ("stars", g.stars.to_string()),
        ]);
    }
    if let Some(e) = &i.github_error {
        rows.push(("github", e.clone()));
    }
    for (k, v) in rows {
        println!("{k:<12}{}", plain(&v));
    }
}

/// Repo-sourced text must not carry terminal escapes.
fn plain(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

fn wt_name(json: bool, issue: u32, project: Option<&str>) -> Result<ExitCode> {
    let projects = refreshed(&Paths::from_env()?)?;
    let query = match project {
        Some(q) => q.to_owned(),
        None => env::current_dir()?.to_string_lossy().into_owned(),
    };
    let p = one(resolve(&projects, &query), &query)?;
    let name = wtname::for_issue(project_dir(&p.id), issue)?;
    emit(json, &name, |n| println!("{}\n{}", n.branch, n.path));
    Ok(ExitCode::SUCCESS)
}

fn init(shell: InitShell) -> Result<ExitCode> {
    print!(
        "{}",
        match shell {
            InitShell::Fish =>
                "function pcd\n    set -l dir (picore path $argv); or return\n    cd $dir\nend\n",
            InitShell::Zsh | InitShell::Bash => {
                "pcd() {\n    local dir\n    dir=\"$(picore path \"$@\")\" || return\n    cd \"$dir\"\n}\n"
            }
        }
    );
    Ok(ExitCode::SUCCESS)
}

fn open(json: bool, name: &str) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    // bad config must not hide the list: drop products, keep going
    let cfg = Config::load(&paths.config_file).unwrap_or_else(|e| {
        eprintln!("warn: {e}: products hidden");
        Config::default()
    });
    let projects = refreshed(&paths)?;
    let path = match target_dir(json, &projects, name)? {
        Ok(p) => p,
        Err(code) => return Ok(code),
    };
    let editor = editor_command(
        cfg.editor.as_deref(),
        env::var("VISUAL").ok().as_deref(),
        code_on_path(),
    )?;
    let (program, args) = editor
        .split_first()
        .expect("editor_command rejects empty argv");
    let mut cmd = Command::new(program);
    cmd.args(args).arg(&path);
    let ok = if io::stdout().is_terminal() {
        cmd.status()?.success()
    } else {
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        true
    };
    emit(
        json,
        &OpenReport {
            path: &path,
            editor: &editor,
        },
        |r| println!("{}", r.path.display()),
    );
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn code_on_path() -> bool {
    find_on_path("code").is_some()
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

fn gui(json: bool) -> Result<ExitCode> {
    let Some(bin) = find_on_path(GUI_BIN) else {
        return Ok(fail(
            json,
            "not_found",
            &format!("{GUI_BIN} not found on PATH: install the GUI bundle"),
            &[],
        ));
    };
    Command::new(bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(ExitCode::SUCCESS)
}

fn one<'a>(r: Resolution<'a>, query: &str) -> Result<&'a Project> {
    r.into_target(query).map(|t| t.project)
}

fn pick(hits: Vec<&Project>) -> Result<&Project> {
    for (i, p) in hits.iter().enumerate() {
        eprintln!("  {}) {}", i + 1, qualified(p));
    }
    eprint!("pick [1-{}]: ", hits.len());
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let choice = line
        .trim()
        .parse::<usize>()
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|i| hits.get(i).copied());
    choice.ok_or_else(|| Error::Ambiguous(hits.iter().map(|p| qualified(p)).collect()))
}

#[derive(Serialize)]
struct StatusRow<'a> {
    name: &'a str,
    path: &'a Path,
    health: ProjectHealth,
}

#[derive(Serialize)]
struct StatusReport<'a> {
    machine: Vec<Check>,
    projects: Vec<StatusRow<'a>>,
}

fn status(json: bool, name: Option<&str>, remote: bool) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    let projects = refreshed(&paths)?;
    let found;
    let selected: Vec<&Project> = match name {
        // Path form, so a Project name never resolves via the cwd.
        Some(q) if Path::new(q).is_absolute() || q.starts_with('.') => {
            let index = Index::open(&paths.index_file)?;
            found = index
                .find(Path::new(q))?
                .ok_or_else(|| Error::NotFound(q.to_owned()))?;
            vec![&found]
        }
        Some(q) => vec![one(resolve(&projects, q), q)?],
        None => projects.iter().collect(),
    };
    let rows = selected
        .iter()
        .map(|p| {
            project_health(p, remote).map(|health| StatusRow {
                name: &p.name,
                path: p.work_tree.as_deref().unwrap_or(p.id.0.as_path()),
                health,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let report = StatusReport {
        machine: machine_health(),
        projects: rows,
    };
    emit(json, &report, |r| {
        for c in &r.machine {
            println!("{}: {:?} {}", c.name, c.status, c.message);
        }
        for p in &r.projects {
            if let Some(c) = &p.health.remote {
                println!("{}: remote {:?} {}", p.name, c.status, c.message);
            }
            for w in &p.health.worktrees {
                let sync = match (w.ahead, w.behind) {
                    (Some(a), Some(b)) => format!("ahead {a} behind {b}"),
                    _ => "no upstream".to_owned(),
                };
                println!("{} {}: dirty {} {sync}", p.name, w.path.display(), w.dirty);
            }
        }
    });
    Ok(ExitCode::SUCCESS)
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn assess_all<'a>(
    projects: &[&'a Project],
    cfg: &Config,
) -> Result<Vec<(&'a Project, Assessment)>> {
    let now = now();
    let mut out = Vec::new();
    for p in projects {
        for wt in &p.worktrees {
            out.push((*p, prune::assess(p, wt, cfg, now)?));
        }
    }
    Ok(out)
}

#[derive(Serialize)]
struct WorktreeRow<'a> {
    project: &'a str,
    #[serde(flatten)]
    assessment: &'a Assessment,
}

fn print_row(r: &WorktreeRow) {
    println!(
        "{}\t{}\t{:?}",
        r.project,
        r.assessment.path.display(),
        r.assessment.verdict
    );
}

fn worktrees(json: bool, name: Option<&str>) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    // bad config must not hide the list: drop products, keep going
    let cfg = Config::load(&paths.config_file).unwrap_or_else(|e| {
        eprintln!("warn: {e}: products hidden");
        Config::default()
    });
    let projects = refreshed(&paths)?;
    let selected: Vec<&Project> = match name {
        Some(q) => vec![one(resolve(&projects, q), q)?],
        None => projects.iter().collect(),
    };
    let assessed = assess_all(&selected, &cfg)?;
    let rows: Vec<WorktreeRow> = assessed
        .iter()
        .map(|(p, a)| WorktreeRow {
            project: &p.name,
            assessment: a,
        })
        .collect();
    emit(json, &rows, |rows| {
        for r in rows {
            print_row(r);
        }
    });
    Ok(ExitCode::SUCCESS)
}

#[derive(Serialize)]
struct PruneReport<'a> {
    dry_run: bool,
    pruned: Vec<WorktreeRow<'a>>,
}

fn prune_cmd(json: bool, target: Option<&str>, yes: bool, force: bool) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    // bad config must not hide the list: drop products, keep going
    let cfg = Config::load(&paths.config_file).unwrap_or_else(|e| {
        eprintln!("warn: {e}: products hidden");
        Config::default()
    });
    let projects = refreshed(&paths)?;
    let all: Vec<&Project> = projects.iter().collect();
    let assessed = assess_all(&all, &cfg)?;
    let picked: Vec<(&Project, Assessment)> = match target {
        None => assessed
            .into_iter()
            .filter(|(_, a)| matches!(a.verdict, Verdict::Prunable(_)))
            .collect(),
        Some(t) => {
            let path = fs::canonicalize(t).unwrap_or_else(|_| PathBuf::from(t));
            let mut hits: Vec<_> = assessed
                .into_iter()
                .filter(|(_, a)| a.path == path || a.branch.as_deref() == Some(t))
                .collect();
            if hits.len() > 1 {
                let candidates = hits.iter().map(|(_, a)| a.path.display().to_string());
                return Err(Error::Ambiguous(candidates.collect()));
            }
            let Some((p, a)) = hits.pop() else {
                return Err(Error::NotFound(t.to_owned()));
            };
            if let Verdict::Kept(b) = &a.verdict {
                // A branch name can hit an unexpected Project: force needs the path.
                let refusal = if !force || matches!(b, Blocker::Main | Blocker::Locked) {
                    Some(format!("blocked by {b:?}"))
                } else {
                    (a.path != path).then(|| "--force needs the Worktree path".to_owned())
                };
                if let Some(why) = refusal {
                    let msg = format!("{}: {why}", a.path.display());
                    return Ok(fail(json, "refused", &msg, &[]));
                }
            }
            vec![(p, a)]
        }
    };
    if yes {
        for (p, a) in &picked {
            prune::remove(p, a, force)?;
        }
    }
    let report = PruneReport {
        dry_run: !yes,
        pruned: picked
            .iter()
            .map(|(p, a)| WorktreeRow {
                project: &p.name,
                assessment: a,
            })
            .collect(),
    };
    emit(json, &report, |r| {
        if r.dry_run {
            println!("dry run: nothing removed (use --yes)");
        }
        for row in &r.pruned {
            print_row(row);
        }
    });
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use picore_core::model::ProjectId;

    use super::*;

    fn project(name: &str) -> Project {
        Project {
            id: ProjectId(PathBuf::from(format!("/x/{name}/.git"))),
            name: name.into(),
            work_tree: None,
            branch: None,
            repo: None,
            techs: vec![],
            worktrees: vec![],
            stamp: 0,
        }
    }

    #[test]
    fn plain_strips_control_chars() {
        assert_eq!(plain("\x1b[31mx\u{9b}\n"), "?[31mx??");
    }

    #[test]
    fn groups_products_then_members_then_ungrouped_by_name() {
        let ps: Vec<Project> = ["zed", "b", "a", "c", "loose"].map(project).into();
        let products = [Some("shop"), Some("app"), Some("app"), None, None];
        let listed: Vec<Listed> = ps
            .iter()
            .zip(products)
            .map(|(project, product)| Listed { project, product })
            .collect();
        let got: Vec<(Option<&str>, Vec<&str>)> = grouped(&listed)
            .into_iter()
            .map(|(p, m)| (p, m.iter().map(|l| l.project.name.as_str()).collect()))
            .collect();
        assert_eq!(
            got,
            [
                (Some("app"), vec!["a", "b"]),
                (Some("shop"), vec!["zed"]),
                (None, vec!["c", "loose"]),
            ]
        );
    }
}
