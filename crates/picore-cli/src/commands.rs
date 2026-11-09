use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

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
use picore_core::view::{self, ProjectView};
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
        e => fail(json, e.code(), &e.to_string(), &[]),
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

struct Loaded {
    cfg: Config,
    projects: Vec<Project>,
}

/// Config (lenient: a bad one must not hide the Index) + refreshed Projects.
fn load() -> Result<Loaded> {
    let paths = Paths::from_env()?;
    let (cfg, bad) = Config::load_or_default(&paths.config_file);
    if let Some(e) = bad {
        eprintln!("warn: {e}: products hidden");
    }
    let (projects, failed) = refresh_all(&Index::open(&paths.index_file)?, &cfg)?;
    for (id, e) in failed {
        eprintln!("warn: {}: {e}", id.0.display());
    }
    Ok(Loaded { cfg, projects })
}

fn exit(failed: bool) -> ExitCode {
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn list(json: bool) -> Result<ExitCode> {
    let l = load()?;
    if l.projects.is_empty() {
        eprintln!("no projects indexed: run `picore index`");
    }
    emit(json, &view::list(&l.projects, &l.cfg), |v| print_table(v));
    Ok(ExitCode::SUCCESS)
}

/// Products by name, members by name, then ungrouped Projects by name.
fn grouped(listed: &[ProjectView]) -> Vec<(Option<&str>, Vec<&ProjectView>)> {
    let mut by_product: BTreeMap<Option<&str>, Vec<&ProjectView>> = BTreeMap::new();
    for l in listed {
        by_product.entry(l.product.as_deref()).or_default().push(l);
    }
    let mut groups: Vec<_> = by_product.into_iter().collect();
    // BTreeMap puts None first: ungrouped goes last
    let shift = usize::from(groups.first().is_some_and(|g| g.0.is_none()));
    groups.rotate_left(shift);
    for (_, members) in &mut groups {
        members.sort_by(|a, b| a.name.cmp(&b.name));
    }
    groups
}

fn print_table(listed: &[ProjectView]) {
    if listed.is_empty() {
        return;
    }
    let header = ["NAME", "PRODUCT", "BRANCH", "WT", "TECHS", "REPO", "PATH"].map(String::from);
    let mut lines: Vec<(Option<&str>, [String; 7])> = vec![(None, header)];
    for (product, members) in grouped(listed) {
        if let Some(name) = product {
            lines.push((Some(name), Default::default()));
        }
        for p in members {
            let indent = if product.is_some() { "  " } else { "" };
            let dash = |s: Option<&str>| s.unwrap_or("-").to_owned();
            lines.push((
                None,
                [
                    format!("{indent}{}", p.name),
                    dash(p.product.as_deref()),
                    dash(p.branch.as_deref()),
                    p.worktrees.to_string(),
                    if p.techs.is_empty() {
                        "-".into()
                    } else {
                        p.techs.join(",")
                    },
                    dash(p.repo.as_deref()),
                    p.id.display().to_string(),
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

/// The one CLI resolver: name, `parent/name`, path or `name@N`; a TTY picks among candidates.
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
    let projects = load()?.projects;
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
    let l = load()?;
    let t = target(json, &l.projects, query)?;
    let report = info::info(&t, l.cfg.product_of(project_dir(&t.project.id)))?;
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
    let projects = load()?.projects;
    let query = match project {
        Some(q) => q.to_owned(),
        None => env::current_dir()?.to_string_lossy().into_owned(),
    };
    let p = target(json, &projects, &query)?.project;
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
    let Loaded { cfg, projects } = load()?;
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
    #[serde(flatten)]
    health: ProjectHealth,
}

#[derive(Serialize)]
struct StatusReport<'a> {
    machine: Vec<Check>,
    projects: Vec<StatusRow<'a>>,
}

fn status(json: bool, name: Option<&str>, remote: bool) -> Result<ExitCode> {
    let projects = load()?.projects;
    let selected: Vec<&Project> = match name {
        Some(q) => vec![target(json, &projects, q)?.project],
        None => projects.iter().collect(),
    };
    let rows: Vec<StatusRow> = selected
        .iter()
        .map(|p| StatusRow {
            name: &p.name,
            path: p.path(),
            health: project_health(p, remote),
        })
        .collect();
    let failed = rows
        .iter()
        .any(|r| r.health.worktrees.iter().any(|w| w.error.is_some()));
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
                let state = match (&w.error, w.dirty, w.ahead, w.behind) {
                    (Some(e), ..) => format!("error {e}"),
                    (None, d, Some(a), Some(b)) => {
                        format!("dirty {} ahead {a} behind {b}", d.unwrap_or(0))
                    }
                    (None, d, ..) => format!("dirty {} no upstream", d.unwrap_or(0)),
                };
                println!("{} {}: {state}", p.name, w.path.display());
            }
        }
    });
    Ok(exit(failed))
}

fn assess_all<'a>(projects: &[&'a Project], cfg: &Config) -> Vec<(&'a Project, Assessment)> {
    projects
        .iter()
        .flat_map(|p| {
            prune::assess_project(p, cfg)
                .into_iter()
                .map(move |a| (*p, a))
        })
        .collect()
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

fn unknown(a: &Assessment) -> bool {
    matches!(a.verdict, Verdict::Unknown(_))
}

fn worktrees(json: bool, name: Option<&str>) -> Result<ExitCode> {
    let Loaded { cfg, projects } = load()?;
    let selected: Vec<&Project> = match name {
        Some(q) => vec![target(json, &projects, q)?.project],
        None => projects.iter().collect(),
    };
    let assessed = assess_all(&selected, &cfg);
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
    Ok(exit(assessed.iter().any(|(_, a)| unknown(a))))
}

#[derive(Serialize)]
struct PruneRow<'a> {
    #[serde(flatten)]
    row: WorktreeRow<'a>,
    removed: bool,
    error: Option<String>,
}

#[derive(Serialize)]
struct PruneReport<'a> {
    dry_run: bool,
    pruned: Vec<PruneRow<'a>>,
}

fn prune_cmd(json: bool, target: Option<&str>, yes: bool, force: bool) -> Result<ExitCode> {
    let Loaded { cfg, projects } = load()?;
    let all: Vec<&Project> = projects.iter().collect();
    let assessed = assess_all(&all, &cfg);
    let picked: Vec<(&Project, Assessment)> = match target {
        // Unknown rows are listed so a failed assessment is never silent.
        None => assessed
            .into_iter()
            .filter(|(_, a)| matches!(a.verdict, Verdict::Prunable(_) | Verdict::Unknown(_)))
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
            // A branch name can hit an unexpected Project: force needs the path.
            let refusal = match &a.verdict {
                Verdict::Prunable(_) => None,
                Verdict::Unknown(why) => Some(format!("cannot assess: {why}")),
                Verdict::Kept(b) if !force || matches!(b, Blocker::Main | Blocker::Locked) => {
                    Some(format!("blocked by {b:?}"))
                }
                Verdict::Kept(_) => {
                    (a.path != path).then(|| "--force needs the Worktree path".to_owned())
                }
            };
            if let Some(why) = refusal {
                let msg = format!("{}: {why}", a.path.display());
                return Ok(fail(json, "refused", &msg, &[]));
            }
            vec![(p, a)]
        }
    };
    let rows: Vec<PruneRow> = picked
        .iter()
        .map(|(p, a)| {
            let outcome = (yes && !unknown(a)).then(|| prune::remove(p, a, force));
            PruneRow {
                row: WorktreeRow {
                    project: &p.name,
                    assessment: a,
                },
                removed: matches!(outcome, Some(Ok(()))),
                error: outcome.and_then(Result::err).map(|e| e.to_string()),
            }
        })
        .collect();
    let failed = rows
        .iter()
        .any(|r| r.error.is_some() || unknown(r.row.assessment));
    let report = PruneReport {
        dry_run: !yes,
        pruned: rows,
    };
    emit(json, &report, |r| {
        if r.dry_run {
            println!("dry run: nothing removed (use --yes)");
        }
        for row in &r.pruned {
            print_row(&row.row);
            if let Some(e) = &row.error {
                eprintln!("error: {}: {e}", row.row.assessment.path.display());
            }
        }
    });
    Ok(exit(failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(name: &str, product: Option<&str>) -> ProjectView {
        ProjectView {
            id: PathBuf::from(format!("/x/{name}")),
            name: name.into(),
            product: product.map(str::to_owned),
            branch: None,
            repo: None,
            techs: vec![],
            worktrees: 1,
        }
    }

    #[test]
    fn plain_strips_control_chars() {
        assert_eq!(plain("\x1b[31mx\u{9b}\n"), "?[31mx??");
    }

    #[test]
    fn groups_products_then_members_then_ungrouped_by_name() {
        let names = ["zed", "b", "a", "c", "loose"];
        let products = [Some("shop"), Some("app"), Some("app"), None, None];
        let listed: Vec<ProjectView> = names
            .into_iter()
            .zip(products)
            .map(|(n, p)| view(n, p))
            .collect();
        let got: Vec<(Option<&str>, Vec<&str>)> = grouped(&listed)
            .into_iter()
            .map(|(p, m)| (p, m.iter().map(|l| l.name.as_str()).collect()))
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
