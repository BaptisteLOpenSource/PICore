use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn stdio_lists_tools() {
    let dir = TempDir::new().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_picore-mcp"))
        .env("PICORE_CONFIG_DIR", dir.path())
        .env("PICORE_DATA_DIR", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut send = |v: Value| writeln!(stdin, "{v}").unwrap();
    let mut recv = || {
        let mut line = String::new();
        out.read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()
    };

    send(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2024-11-05","capabilities":{},
        "clientInfo":{"name":"t","version":"0"}}}),
    );
    recv();
    send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    let reply = recv();

    let mut names: Vec<&str> = reply["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "get_health",
            "get_project_context",
            "get_project_section",
            "list_projects",
            "list_worktrees"
        ]
    );
    child.kill().unwrap();
    child.wait().unwrap();
}
