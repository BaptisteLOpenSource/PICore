use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde::de::IgnoredAny;

const LANGUAGES: &[(&str, &str)] = &[
    ("Cargo.toml", "rust"),
    ("package.json", "javascript"),
    ("tsconfig.json", "typescript"),
    ("pyproject.toml", "python"),
    ("go.mod", "go"),
    ("composer.json", "php"),
    ("Gemfile", "ruby"),
    ("pom.xml", "java"),
    ("build.gradle", "java"),
    ("build.gradle.kts", "java"),
];

const FRAMEWORKS: &[(&str, &str)] = &[
    ("react", "react"),
    ("solid-js", "solid"),
    ("vue", "vue"),
    ("svelte", "svelte"),
    ("next", "next"),
    ("astro", "astro"),
    ("express", "express"),
    ("@tauri-apps/api", "tauri"),
    ("tauri", "tauri"),
    ("axum", "axum"),
    ("actix-web", "actix-web"),
    ("bevy", "bevy"),
    ("clap", "clap"),
];

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    dependencies: BTreeMap<String, IgnoredAny>,
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: BTreeMap<String, IgnoredAny>,
}

// Malformed manifests yield no deps: the language is still known from the file's presence.
pub fn detect(work_tree: &Path) -> Vec<String> {
    let mut techs: BTreeSet<&str> = LANGUAGES
        .iter()
        .filter(|(file, _)| work_tree.join(file).is_file())
        .map(|(_, tech)| *tech)
        .collect();
    for dep in package_deps(work_tree)
        .into_iter()
        .chain(cargo_deps(work_tree))
    {
        if let Some((_, fw)) = FRAMEWORKS.iter().find(|(name, _)| *name == dep) {
            techs.insert(fw);
        }
    }
    techs.into_iter().map(str::to_owned).collect()
}

fn package_deps(work_tree: &Path) -> Vec<String> {
    fs::read_to_string(work_tree.join("package.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Manifest>(&text).ok())
        .map(manifest_keys)
        .unwrap_or_default()
}

fn cargo_deps(work_tree: &Path) -> Vec<String> {
    fs::read_to_string(work_tree.join("Cargo.toml"))
        .ok()
        .and_then(|text| toml::from_str::<Manifest>(&text).ok())
        .map(manifest_keys)
        .unwrap_or_default()
}

fn manifest_keys(m: Manifest) -> Vec<String> {
    m.dependencies
        .into_keys()
        .chain(m.dev_dependencies.into_keys())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::detect;

    fn dir_with(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for (name, content) in files {
            fs::write(dir.path().join(name), content).unwrap();
        }
        dir
    }

    #[test]
    fn detects_language_and_frameworks() {
        let dir = dir_with(&[
            (
                "package.json",
                r#"{"dependencies":{"solid-js":"1"},"devDependencies":{"astro":"4"}}"#,
            ),
            ("tsconfig.json", "{}"),
        ]);
        assert_eq!(
            detect(dir.path()),
            ["astro", "javascript", "solid", "typescript"]
        );
    }

    #[test]
    fn rust_tauri() {
        let dir = dir_with(&[(
            "Cargo.toml",
            "[package]\nname = \"x\"\n\n[dependencies]\ntauri = \"2\"\n",
        )]);
        assert_eq!(detect(dir.path()), ["rust", "tauri"]);
    }

    #[test]
    fn malformed_manifest_keeps_language() {
        let dir = dir_with(&[("package.json", "{")]);
        assert_eq!(detect(dir.path()), ["javascript"]);
    }

    #[test]
    fn empty_dir_detects_nothing() {
        let dir = TempDir::new().unwrap();
        assert!(detect(dir.path()).is_empty());
    }
}
