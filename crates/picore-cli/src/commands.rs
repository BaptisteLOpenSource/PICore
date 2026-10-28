use std::env;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use picore_core::config::{Config, Paths};
use picore_core::discovery::suggest_roots;
use picore_core::error::{Error, Result};
use picore_core::model::{Project, ProjectId};
use picore_core::refresh::{index_all, refresh_many};
use picore_core::store::Index;

use crate::output::{emit, fail, mascot};
use crate::{Cmd, RootCmd};

pub fn run(json: bool, quiet: bool, cmd: &Cmd) -> ExitCode {
    let result = match cmd {
        Cmd::Root { action } => root(json, action),
        Cmd::Index => index(json, quiet),
        Cmd::List => list(json),
    };
    result.unwrap_or_else(|e| fail(json, code_of(&e), &e.to_string(), &[]))
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

fn list(json: bool) -> Result<ExitCode> {
    let paths = Paths::from_env()?;
    let index = Index::open(&paths.index_file)?;
    let ids: Vec<ProjectId> = index.all()?.into_iter().map(|p| p.id).collect();
    for (id, outcome) in refresh_many(&index, &ids) {
        if let Err(e) = outcome {
            eprintln!("warn: {}: {e}", id.0.display());
        }
    }
    let projects = index.all()?;
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
