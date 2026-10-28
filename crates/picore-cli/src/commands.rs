use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use picore_core::config::{Config, Paths, editor_command};
use picore_core::discovery::suggest_roots;
use picore_core::error::{Error, Result};
use picore_core::health::{Check, ProjectHealth, machine_health, project_health};
use picore_core::model::{Project, ProjectId};
use picore_core::prune::{self, Assessment, Blocker, Verdict};
use picore_core::refresh::{index_all, refresh_many};
use picore_core::resolve::{Resolution, qualified, resolve};
use picore_core::store::Index;
use serde::Serialize;

use crate::output::{emit, fail, mascot};
use crate::{Cmd, RootCmd};

pub fn run(json: bool, quiet: bool, cmd: &Cmd) -> ExitCode {
    let result = match cmd {
        Cmd::Root { action } => root(json, action),
        Cmd::Index => index(json, quiet),
        Cmd::List => list(json),
        Cmd::Open { name } => open(json, name),
        Cmd::Status { name, remote } => status(json, name.as_deref(), *remote),
        Cmd::Worktrees { name } => worktrees(json, name.as_deref()),
        Cmd::Prune { target, yes, force } => prune_cmd(json, target.as_deref(), *yes, *force),
    };
    result.unwrap_or_else(|e| match e {
        Error::Ambiguous(candidates) => fail(json, "ambiguous", "ambiguous name", &candidates),
        e => fail(json, code_of(&e), &e.to_string(), &[]),
    })
}

fn code_of(e: &Error) -> &'static str {
    match e {
        Error::Proc(_) => "git",
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
    let index = Index::open(&paths.index_file)?;
    let ids: Vec<ProjectId> = index.all()?.into_iter().map(|p| p.id).collect();
    for (id, outcome) in refresh_many(&index, &ids) {
        if let Err(e) = outcome {
            eprintln!("warn: {}: {e}", id.0.display());
        }
    }
    index.all()
}

fn list(json: bool) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    let projects = refreshed(&paths)?;
    if projects.is_empty() {
        eprintln!("no projects indexed: run `picore index`");
    }
    emit(json, &projects, |p| print_table(p));
    Ok(ExitCode::SUCCESS)
}

fn print_table(projects: &[Project]) {
    if projects.is_empty() {
        return;
    }
    let mut rows = vec![["NAME", "BRANCH", "WT", "TECHS", "PATH"].map(String::from)];
    rows.extend(projects.iter().map(|p| {
        [
            p.name.clone(),
            p.branch.clone().unwrap_or_else(|| "-".into()),
            p.worktrees.len().to_string(),
            if p.techs.is_empty() {
                "-".into()
            } else {
                p.techs.join(",")
            },
            p.work_tree
                .as_deref()
                .unwrap_or(&p.id.0)
                .display()
                .to_string(),
        ]
    }));
    let widths: [usize; 5] =
        std::array::from_fn(|c| rows.iter().map(|r| r[c].chars().count()).max().unwrap_or(0));
    for row in rows {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(c, cell)| {
                if c == 4 {
                    cell.clone()
                } else {
                    format!("{cell:<w$}", w = widths[c])
                }
            })
            .collect();
        println!("{}", line.join("  "));
    }
}

#[derive(Serialize)]
struct OpenReport<'a> {
    path: &'a Path,
    editor: &'a [String],
}

fn open(json: bool, name: &str) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    let cfg = Config::load(&paths.config_file)?;
    let projects = refreshed(&paths)?;
    let project = match resolve(&projects, name) {
        Resolution::Many(hits) if io::stdin().is_terminal() => pick(hits)?,
        r => one(r, name)?,
    };
    let Some(path) = project
        .work_tree
        .clone()
        .or_else(|| project.linked_worktrees().next().map(|w| w.path.clone()))
    else {
        return Ok(fail(
            json,
            "refused",
            &format!("{}: no worktree to open", project.name),
            &[],
        ));
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
    env::var_os("PATH").is_some_and(|p| env::split_paths(&p).any(|d| d.join("code").is_file()))
}

fn one<'a>(r: Resolution<'a>, query: &str) -> Result<&'a Project> {
    match r {
        Resolution::One(p) => Ok(p),
        Resolution::Many(hits) => Err(Error::Ambiguous(hits.into_iter().map(qualified).collect())),
        Resolution::None => Err(Error::NotFound(query.to_owned())),
    }
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
    let selected: Vec<&Project> = match name {
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
            println!(
                "{}: dirty {} ahead {} behind {}",
                p.name, p.health.dirty, p.health.ahead, p.health.behind
            );
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
    stale_days: u32,
) -> Result<Vec<(&'a Project, Assessment)>> {
    let now = now();
    let mut out = Vec::new();
    for p in projects {
        for wt in &p.worktrees {
            out.push((*p, prune::assess(p, wt, stale_days, now)?));
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

fn worktrees(json: bool, name: Option<&str>) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    let cfg = Config::load(&paths.config_file)?;
    let projects = refreshed(&paths)?;
    let selected: Vec<&Project> = match name {
        Some(q) => vec![one(resolve(&projects, q), q)?],
        None => projects.iter().collect(),
    };
    let assessed = assess_all(&selected, cfg.stale_days)?;
    let rows: Vec<WorktreeRow> = assessed
        .iter()
        .map(|(p, a)| WorktreeRow {
            project: &p.name,
            assessment: a,
        })
        .collect();
    emit(json, &rows, |rows| {
        for r in rows {
            println!(
                "{}\t{}\t{:?}",
                r.project,
                r.assessment.path.display(),
                r.assessment.verdict
            );
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
    let cfg = Config::load(&paths.config_file)?;
    let projects = refreshed(&paths)?;
    let all: Vec<&Project> = projects.iter().collect();
    let assessed = assess_all(&all, cfg.stale_days)?;
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
            if let Verdict::Kept(b) = &a.verdict
                && (!force || matches!(b, Blocker::Main | Blocker::Locked))
            {
                let msg = format!("{}: blocked by {b:?}", a.path.display());
                return Ok(fail(json, "refused", &msg, &[]));
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
            println!("{}\t{}", row.project, row.assessment.path.display());
        }
    });
    Ok(ExitCode::SUCCESS)
}
