use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_file: PathBuf,
    pub index_file: PathBuf,
}

impl Paths {
    pub fn from_env() -> Result<Paths> {
        let config_dir = match env::var_os("PICORE_CONFIG_DIR") {
            Some(d) => PathBuf::from(d),
            None => project_dirs()?.config_dir().to_path_buf(),
        };
        let data_dir = match env::var_os("PICORE_DATA_DIR") {
            Some(d) => PathBuf::from(d),
            None => project_dirs()?.data_dir().to_path_buf(),
        };
        Ok(Paths {
            config_file: config_dir.join("config.toml"),
            index_file: data_dir.join("picore.db"),
        })
    }
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "picore").ok_or_else(|| Error::Config("no home directory".into()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub roots: Vec<PathBuf>,
    pub max_depth: usize,
    pub ignore_dirs: Vec<String>,
    pub editor: Option<String>,
    pub stale_days: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            roots: Vec::new(),
            max_depth: 4,
            ignore_dirs: ["node_modules", "target", ".venv"].map(String::from).into(),
            editor: None,
            stale_days: 30,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| Error::Config(e.to_string())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| Error::Config(e.to_string()))?;
        fs::write(path, text)?;
        Ok(())
    }

    pub fn add_root(&mut self, p: &Path) -> Result<bool> {
        let canon = p.canonicalize()?;
        if self.roots.contains(&canon) {
            return Ok(false);
        }
        self.roots.push(canon);
        Ok(true)
    }

    pub fn remove_root(&mut self, p: &Path) -> bool {
        let p = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let before = self.roots.len();
        self.roots.retain(|r| *r != p);
        self.roots.len() != before
    }
}

pub fn editor_command(
    configured: Option<&str>,
    visual: Option<&str>,
    code_on_path: bool,
) -> Result<Vec<String>> {
    let cmd = configured
        .or(visual)
        .or(code_on_path.then_some("code"))
        .ok_or_else(|| Error::Config("no editor: set `editor` in config.toml or $VISUAL".into()))?;
    let argv = shell_words::split(cmd).map_err(|e| Error::Config(e.to_string()))?;
    if argv.is_empty() {
        return Err(Error::Config("empty editor command".into()));
    }
    Ok(argv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let c = Config::load(Path::new("/nonexistent/config.toml")).unwrap();
        assert_eq!((c.max_depth, c.stale_days, c.ignore_dirs.len()), (4, 30, 3));
    }

    #[test]
    fn roundtrip_and_dedupe_roots() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("c/config.toml");
        let mut c = Config::default();
        assert!(c.add_root(d.path()).unwrap());
        assert!(!c.add_root(d.path()).unwrap());
        c.save(&f).unwrap();
        assert_eq!(Config::load(&f).unwrap().roots.len(), 1);
    }

    #[test]
    fn editor_precedence_and_split() {
        assert_eq!(
            editor_command(Some("nvim -u custom.lua"), Some("vim"), true).unwrap(),
            ["nvim", "-u", "custom.lua"]
        );
        assert_eq!(editor_command(None, Some("vim"), true).unwrap(), ["vim"]);
        assert_eq!(editor_command(None, None, true).unwrap(), ["code"]);
        assert!(editor_command(None, None, false).is_err());
    }
}
