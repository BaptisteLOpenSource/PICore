use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs};
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
pub struct Product {
    pub path: PathBuf,
    #[serde(skip)]
    resolved: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub roots: Vec<PathBuf>,
    pub max_depth: usize,
    pub ignore_dirs: Vec<String>,
    pub editor: Option<String>,
    pub stale_days: u32,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub products: BTreeMap<String, Product>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            roots: Vec::new(),
            max_depth: 4,
            ignore_dirs: ["node_modules", "target", ".venv"].map(String::from).into(),
            editor: None,
            stale_days: 30,
            products: BTreeMap::new(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        match fs::read_to_string(path) {
            Ok(text) => {
                let mut config: Config =
                    toml::from_str(&text).map_err(|e| Error::Config(e.to_string()))?;
                config.resolve_products()?;
                Ok(config)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    fn resolve_products(&mut self) -> Result<()> {
        for (name, product) in self.products.iter_mut() {
            product.resolved = resolve_product_path(name, &product.path)?;
        }
        let products: Vec<(&String, &PathBuf)> = self
            .products
            .iter()
            .map(|(name, p)| (name, &p.resolved))
            .collect();
        for (i, (a, pa)) in products.iter().enumerate() {
            for (b, pb) in &products[i + 1..] {
                if pa.starts_with(pb) || pb.starts_with(pa) {
                    return Err(Error::Config(format!("product folders nest: {a} and {b}")));
                }
            }
        }
        Ok(())
    }

    pub fn product_of(&self, dir: &Path) -> Option<&str> {
        self.products
            .iter()
            .find(|(_, p)| dir.starts_with(&p.resolved))
            .map(|(name, _)| name.as_str())
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

    pub fn add_product(&mut self, name: &str, path: &Path) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Config("product name is empty".into()));
        }
        if self.products.contains_key(name) {
            return Err(Error::Config(format!("product {name} already exists")));
        }
        let mut next = self.clone();
        next.products.insert(
            name.to_owned(),
            Product {
                path: path.to_path_buf(),
                resolved: PathBuf::new(),
            },
        );
        next.resolve_products()?;
        *self = next;
        Ok(())
    }

    pub fn remove_product(&mut self, name: &str) -> bool {
        self.products.remove(name).is_some()
    }
}

fn resolve_product_path(name: &str, path: &Path) -> Result<PathBuf> {
    let expanded = match path.strip_prefix("~") {
        Ok(rest) => {
            let home = BaseDirs::new().ok_or_else(|| Error::Config("no home directory".into()))?;
            home.home_dir().join(rest)
        }
        Err(_) => path.to_path_buf(),
    };
    if !expanded.is_absolute() {
        return Err(Error::Config(format!(
            "product {name}: path must be absolute or start with ~/"
        )));
    }
    Ok(expanded.canonicalize().unwrap_or(expanded))
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

    fn write_config(dir: &Path, body: &str) -> PathBuf {
        let f = dir.join("config.toml");
        fs::write(&f, body).unwrap();
        f
    }

    #[test]
    fn products_parse_and_match_by_folder() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("alpha"), d.path().join("beta"));
        fs::create_dir_all(a.join("app")).unwrap();
        fs::create_dir_all(&b).unwrap();
        let f = write_config(
            d.path(),
            &format!(
                "[products.alpha]\npath = \"{}\"\n[products.beta]\npath = \"{}\"\n",
                a.display(),
                b.display()
            ),
        );
        let c = Config::load(&f).unwrap();
        assert_eq!(
            c.product_of(&a.join("app").canonicalize().unwrap()),
            Some("alpha")
        );
        assert_eq!(c.product_of(&b.canonicalize().unwrap()), Some("beta"));
        assert_eq!(c.product_of(&a.canonicalize().unwrap()), Some("alpha"));
        assert_eq!(c.product_of(d.path()), None);
    }

    #[test]
    fn add_product_validates_like_load() {
        let d = tempfile::tempdir().unwrap();
        let outer = d.path().join("outer");
        fs::create_dir_all(outer.join("inner")).unwrap();
        let mut c = Config::default();
        c.add_product("outer", &outer).unwrap();
        assert!(
            matches!(c.add_product("outer", d.path()), Err(Error::Config(m)) if m.contains("already exists"))
        );
        assert!(
            matches!(c.add_product("inner", &outer.join("inner")), Err(Error::Config(m)) if m.starts_with("product folders nest: "))
        );
        assert!(
            matches!(c.add_product(" ", d.path()), Err(Error::Config(m)) if m.contains("empty"))
        );
        assert!(matches!(
            c.add_product("rel", Path::new("rel")),
            Err(Error::Config(_))
        ));
        assert_eq!(c.products.len(), 1);
        let f = d.path().join("config.toml");
        c.save(&f).unwrap();
        let inner = outer.join("inner").canonicalize().unwrap();
        assert_eq!(Config::load(&f).unwrap().product_of(&inner), Some("outer"));
        assert!(c.remove_product("outer"));
        assert!(!c.remove_product("outer"));
    }

    #[test]
    fn nested_product_folders_are_rejected() {
        let d = tempfile::tempdir().unwrap();
        let outer = d.path().join("outer");
        let inner = outer.join("inner");
        fs::create_dir_all(&inner).unwrap();
        let f = write_config(
            d.path(),
            &format!(
                "[products.outer]\npath = \"{}\"\n[products.inner]\npath = \"{}\"\n",
                outer.display(),
                inner.display()
            ),
        );
        assert!(
            matches!(Config::load(&f), Err(Error::Config(m)) if m.starts_with("product folders nest: ") && m.contains("outer") && m.contains("inner"))
        );
    }

    #[test]
    fn tilde_expands_to_home() {
        let d = tempfile::tempdir().unwrap();
        let f = write_config(
            d.path(),
            "[products.p]\npath = \"~/picore-missing-product\"\n",
        );
        let home = BaseDirs::new().unwrap().home_dir().to_path_buf();
        let c = Config::load(&f).unwrap();
        assert_eq!(
            c.product_of(&home.join("picore-missing-product")),
            Some("p")
        );
    }

    #[test]
    fn save_keeps_user_product_path() {
        let d = tempfile::tempdir().unwrap();
        let f = write_config(d.path(), "[products.p]\npath = \"~/x\"\n");
        Config::load(&f).unwrap().save(&f).unwrap();
        assert!(fs::read_to_string(&f).unwrap().contains("\"~/x\""));
    }

    #[test]
    fn empty_or_relative_product_path_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        for bad in ["", "rel/dir"] {
            let f = write_config(d.path(), &format!("[products.p]\npath = \"{bad}\"\n"));
            assert!(
                matches!(Config::load(&f), Err(Error::Config(m)) if m.contains("product p: path must be absolute"))
            );
        }
    }

    #[test]
    fn symlinked_product_matches_canonical_project_dir() {
        let d = tempfile::tempdir().unwrap();
        let real = d.path().join("real");
        let project = real.join("proj");
        fs::create_dir_all(&project).unwrap();
        let link = d.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let f = write_config(
            d.path(),
            &format!("[products.p]\npath = \"{}\"\n", link.display()),
        );
        let c = Config::load(&f).unwrap();
        assert_eq!(c.product_of(&project.canonicalize().unwrap()), Some("p"));
    }
}
