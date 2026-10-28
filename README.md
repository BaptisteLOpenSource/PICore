<div align="center">

```
  (o>   PICore
  //\   Project Intelligence Core
  V_/_
```

**A local index of your git projects and worktrees: health checks, safe cleanup,
and condensed context for AI agents over MCP.**

![Status](https://img.shields.io/badge/status-pre--alpha-%23F5A623.svg?style=for-the-badge)
![License](https://img.shields.io/badge/license-MIT-%23007ACC.svg?style=for-the-badge)
![Linux](https://img.shields.io/badge/linux-%23FCC624.svg?style=for-the-badge&logo=linux&logoColor=black)

</div>

---

## Why

You juggle dozens of repositories, and AI agents keep spawning worktrees all over your disk.
PICore answers, in one command:

- **Which projects do I have, and where are their worktrees?** Even the ones an agent created in `/tmp`.
- **Does my git and GitHub access actually work?** Checked with *your* `git`, `ssh-agent` and `gh`.
- **What can I clean up safely?** Only merged, clean worktrees. Never your uncommitted work.
- **What should an agent know about this project?** A small, whitelisted context instead of a full tree crawl.

PICore guides your AI sessions. It does not run them.

PICore has no telemetry. It sends nothing to its authors.

## Stack <img src="https://media2.giphy.com/media/QssGEmpkyEOhBCb7e1/giphy.gif?cid=ecf05e47a0n3gi1bfqntqmob8g9aid1oyj2wr3ds3mg700bl&rid=giphy.gif" height="30">

**Core (MVP)**

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![Clap](https://img.shields.io/badge/clap-%23CE422B.svg?style=for-the-badge&logo=rust&logoColor=white)
![SQLite](https://img.shields.io/badge/sqlite-%2307405e.svg?style=for-the-badge&logo=sqlite&logoColor=white)
![Git](https://img.shields.io/badge/git-%23F05033.svg?style=for-the-badge&logo=git&logoColor=white)
![GitHub CLI](https://img.shields.io/badge/github%20cli-%23121011.svg?style=for-the-badge&logo=github&logoColor=white)
![MCP](https://img.shields.io/badge/MCP-%23191919.svg?style=for-the-badge&logo=modelcontextprotocol&logoColor=white)

**Desktop and IDE (V1)**

![Tauri](https://img.shields.io/badge/tauri-%2324C8DB.svg?style=for-the-badge&logo=tauri&logoColor=%23FFFFFF)
![SolidJS](https://img.shields.io/badge/solidjs-%232c4f7c.svg?style=for-the-badge&logo=solid&logoColor=white)
![UnoCSS](https://img.shields.io/badge/unocss-%23333333.svg?style=for-the-badge&logo=unocss&logoColor=white)
![TypeScript](https://img.shields.io/badge/typescript-%23007ACC.svg?style=for-the-badge&logo=typescript&logoColor=white)
![pnpm](https://img.shields.io/badge/pnpm-%234a4a4a.svg?style=for-the-badge&logo=pnpm&logoColor=f69220)
![VS Code](https://img.shields.io/badge/VS%20Code-0078d7.svg?style=for-the-badge&logo=visual-studio-code&logoColor=white)

## Components

| Component | What | Milestone |
|---|---|---|
| `picore-core` | Index, discovery, health, context (library) | MVP |
| `picore` | CLI | MVP |
| `picore-mcp` | Read-only MCP server (stdio) | MVP |
| GUI | Desktop dashboard (Tauri + SolidJS) | V1 |
| VS Code extension | Show / hide root config files | V1 |
| Website | Docs and landing page | V2 |

## Install

Requires a Rust toolchain (pinned in `rust-toolchain.toml`), `git`, and optionally `gh`.
From a clone of this repository:

```sh
cargo install --path crates/picore-cli    # binary: picore
cargo install --path crates/picore-mcp    # binary: picore-mcp
```

Shell completions (bash, zsh, fish) and a man page are written to stdout:

```sh
picore completions zsh > ~/.zfunc/_picore
picore man > picore.1
```

Config and Index live in XDG paths: `~/.config/picore/config.toml` and
`~/.local/share/picore/picore.db`. Set `PICORE_CONFIG_DIR` and `PICORE_DATA_DIR` to override.

## Usage

Declare where your projects live, then index them. Output below is trimmed, `~` stands for
the home directory.

```sh
picore root add ~/Documents/Git
# ~/Documents/Git
picore index
# indexed 9 projects: 9 updated, 0 removed
picore list
# NAME             PRODUCT  BRANCH         WT  TECHS                             PATH
# PICore           -        dev            14  -                                 ~/Documents/Git/Config/PICore
picore status
# git: Ok git version 2.56.0
# ssh-agent: Ok agent has keys
# PICore: dirty 0 ahead 0 behind 0
picore worktrees PICore
# PICore  ~/Documents/Git/Config/PICore  Kept(Main)
# PICore  ~/Documents/Git/Config/PICore.worktrees/10-name-resolution  Kept(NotMerged)
picore prune
# dry run: nothing removed (use --yes)
# ...one line per Prunable Worktree, omitted
```

`picore prune --yes` removes only Prunable Worktrees. A kept Worktree (unmerged, local changes) is
removed only when you name it by path and pass `--force`. Main and locked Worktrees are
never removed.

Scanning runs git inside your repositories. PICore neutralizes fsmonitor, a repo-local `core.sshCommand` and the
`ext` protocol, but a `filter.*.clean` driver declared through `.gitattributes` and your git config
can still run during `status`. Only scan repositories you trust.

## MCP

`picore-mcp` is a read-only MCP server over stdio. Register it in Claude Code:

```sh
claude mcp add picore picore-mcp
```

Or in any MCP client config:

```json
{ "mcpServers": { "picore": { "command": "picore-mcp" } } }
```

Tools: `list_projects`, `get_project_context`, `get_project_section`, `list_worktrees`,
`get_health`. Project context is read from `README.md`, `CLAUDE.md` and `AGENTS.md` only.
Pass the `id` returned by `list_projects` as the `project` argument.

## Documentation

- [Architecture](ARCHITECTURE.md)
- [Glossary](GLOSSARY.md)
- [Decisions](docs/decisions/)

## License

[MIT](LICENSE) © 2026 Baptiste Dechamp
