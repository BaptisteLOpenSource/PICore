# Contributing

Read [ARCHITECTURE.md](ARCHITECTURE.md), [GLOSSARY.md](GLOSSARY.md) and
[docs/decisions/](docs/decisions/) first. Do not reopen a recorded decision without a new ADR.

## Workflow

- Branch from `dev`, named `<issue-number>-<slug>`.
- Open your pull request against `dev`. `main` only takes reviewed pull requests from `dev`.
- Commits use [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): subject`,
  lowercase, no period, 75 characters at most.

## Before you commit

```sh
just check
```

It runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`. It must pass.
