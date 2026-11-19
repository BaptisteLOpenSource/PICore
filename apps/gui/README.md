# PICore GUI

Desktop view of the PICore Index: Projects grouped by product, Worktrees, Health Checks.

## Stack

Tauri 2 (Rust backend), SolidJS, UnoCSS.

## Boundaries

- Writes only `config.toml` (product groups). Never writes inside a Project.
- `src/api.ts` is the sole importer of `@tauri-apps/*`.
- Views read `src/store.ts`, never the raw source.

## Prerequisites

Rust, Node 22, pnpm (`corepack enable`), `libwebkit2gtk-4.1-dev` on Linux.

## Commands

- `just gui-check`: fmt, clippy, tests, vitest.
- `pnpm tauri dev`: run the app (from `apps/gui`).

Decision: [ADR 0004](../../docs/decisions/0004-product-groups-in-config.md).
