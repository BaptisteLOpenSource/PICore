# picore-core

Library behind `picore` (CLI and `picore mcp`). Bounded context: Projects, Worktrees, the Index,
Health Checks and Project Context.

- Vocabulary: [GLOSSARY.md](../../GLOSSARY.md). Code names are glossary terms.
- Layout and dependency rule: [ARCHITECTURE.md](../../ARCHITECTURE.md).
- Decisions: [0001 workspace](../../docs/decisions/0001-monorepo-cargo-workspace.md),
  [0002 native git](../../docs/decisions/0002-native-git-cli.md),
  [0003 no daemon](../../docs/decisions/0003-no-daemon.md).

## Adapters

| Module | Sole owner of |
|---|---|
| `git.rs` | every `git` call, porcelain output only (`health.rs` still runs `git --version` directly) |
| `gh.rs` | every `gh` call |
| `process.rs` | spawning external binaries: `LC_ALL=C`, `GIT_TERMINAL_PROMPT=0`, timeout |
| `store.rs` | the SQLite Index |
| `config.rs` | `config.toml` and config/data paths (`PICORE_CONFIG_DIR`, `PICORE_DATA_DIR`) |

Domain modules (`model.rs`, `health.rs`, `prune.rs`, `context.rs`) never call `std::process`.

## Invariants

- The Index is a cache keyed by the Project's git dir. Rebuilt from git at any time.
- No background process. Every read goes through `refresh`.
- Health Check results are never stored in the Index.
- Nothing writes inside a Project. The only destructive call is `prune::remove`: Main and
  locked Worktrees are never removed, kept ones need an explicit target and `force`.
- MCP is read-only. Project Context reads only `WHITELIST` (`README.md`, `CLAUDE.md`,
  `AGENTS.md`), with the canonical path checked to stay inside the Project.

## Tests

Run against real `git` in temp dirs. No git mocks. `just check` from the repository root.
