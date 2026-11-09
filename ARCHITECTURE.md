# Architecture

Vocabulary: [GLOSSARY.md](GLOSSARY.md). Decisions: [docs/decisions/](docs/decisions/).

## Layout

```
PICore/
├── crates/picore-core   # library: Index, discovery, health, context. No I/O framework
├── crates/picore-cli    # binary `picore` (clap), `mcp` module = MCP over stdio
└── apps/gui             # V1: Tauri v2 + SolidJS, embeds picore-core
```

Monorepo rationale: [ADR 0001](docs/decisions/0001-monorepo-cargo-workspace.md).

## Dependency rule

`picore-cli` and `apps/gui` depend on `picore-core`, never on each other. Inside `picore-cli`, only
`main.rs` reaches the `mcp` module, and `mcp` imports only `picore-core`.

Every listing goes through `picore_core::view::ProjectView` (CLI `list --json`, MCP `list_projects`,
GUI overview): one id (the Project path), one display name, one Worktree count. A per-Worktree
failure (git error, missing directory) is a row value (`error`, `Verdict::Unknown`), never an
abort of the whole command. JSON shapes: [README](README.md#json-output).

Inside `picore-core`, domain types (Project, Worktree, Health Check, Project Context) know
nothing about SQLite, processes or the filesystem. Adapters own those:

| Adapter | Sole owner of |
|---|---|
| `git` | every `git` invocation ([ADR 0002](docs/decisions/0002-native-git-cli.md)) |
| `gh` | every `gh` invocation |
| `store` | the SQLite Index |
| `config` | `config.toml` |

## Invariants

- The Index is a cache, rebuildable from git at any time. It holds no user-entered data, so a
  Project's identity is its path: moving a repository yields a new Project, losing nothing.
- No background process ([ADR 0003](docs/decisions/0003-no-daemon.md)). Every component goes
  through the same `refresh(project)`.
- Health Check results are never persisted.
- PICore never writes inside a Project. The only destructive action is removing a Worktree
  through `picore prune`, which never touches local changes or unmerged work without an explicit
  path and `--force`.
- Nothing outside the context whitelist is ever read for an AI agent.
- Repo-local `filter.*` drivers are blanked during `status`; `merge.*.driver` of every scope is
  forced to `false` (passed through `GIT_CONFIG_*` env, safe for `=` in names). The prune merge
  check sends merge-tree objects to a scratch dir, never the Project. Residual risk: a global or
  system filter driver still runs when a repository binds it. Scan Roots must hold trusted
  repositories only.

## Data locations (XDG)

- Config: `~/.config/picore/config.toml`
- Index: `~/.local/share/picore/picore.db`
