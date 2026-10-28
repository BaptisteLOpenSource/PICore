use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub PathBuf);

pub fn project_name(id: &ProjectId) -> String {
    let dir = match id.0.file_name() {
        Some(n) if n == ".git" || n == ".bare" => id.0.parent().unwrap_or(&id.0),
        _ => &id.0,
    };
    let name = dir
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    name.strip_suffix(".git").unwrap_or(&name).to_owned()
}
