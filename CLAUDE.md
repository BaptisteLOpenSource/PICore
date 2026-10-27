# PICore

Local index of git Projects + Worktrees, health checks, read-only MCP context for agents.

## Read first

- Vocabulary: `GLOSSARY.md`. Code names = glossary terms (Project, Worktree, Scan Root, Index, Health Check).
- Layout, dependency rule, invariants: `ARCHITECTURE.md`.
- Decisions: `docs/decisions/`. Do not reopen without new ADR.

## Hard rules

- git / gh only via native binaries through `picore_core::process::run`. No gix, no git2 (ADR 0002).
- No daemon (ADR 0003). Reads go through `refresh`.
- Never write inside a Project. Only destructive op = `prune`, guarded.
- MCP read-only. Files for agents: `README.md`, `CLAUDE.md`, `AGENTS.md` only.
- stdout = data. Mascot on stderr, TTY only.
- Tests on real git repos in temp dirs. No git mocks.

## Commands

- `just check` = fmt + clippy `-D warnings` + tests. Run before every commit.

## Workflow

- Branch from `dev`, PR to `dev`, `main` only via reviewed PR from `dev`.
- Conventional commits, lowercase, <= 75 chars.
- Plans/specs in `.superpowers/` (gitignored).
