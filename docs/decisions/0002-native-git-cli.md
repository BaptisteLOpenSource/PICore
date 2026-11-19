# Native git CLI, no git library

PICore talks to git only by running the user's own `git` binary (`std::process::Command`), never
through `gix` or `git2`. PICore's job is to tell the user whether *their* git works: a library
reimplements SSH agents, credential helpers, `insteadOf`, `includeIf` and proxies imperfectly,
so it would report a health different from what the user sees. The same applies to `gh`.

## Considered Options

- **`gix` for local reads, CLI only for network checks**: rejected. Two commands per Project
  (`git status --porcelain=v2 --branch`, `git worktree list --porcelain`) cover the local
  needs; run in parallel, 50 Projects stay well under a second. `gix` adds a large, pre-1.0
  dependency and ignores the user's `core.fsmonitor`.
- **`git2`**: rejected, C dependency and known divergence from git's credential handling.

## Consequences

- Every invocation goes through a single `git` module: `LC_ALL=C`, `GIT_TERMINAL_PROMPT=0`,
  strict timeout, porcelain formats only. Swapping it later touches that module only.
- Tests need a real `git` installed.
