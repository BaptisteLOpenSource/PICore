# Monorepo: Cargo workspace, pnpm workspace added for apps

All components live in one repository: Rust crates under `crates/` (core, CLI, MCP server) and,
from V1, JavaScript/TypeScript apps under `apps/` (GUI frontend, VS Code extension). One repo
means one schema and one core library, with no synchronised pull requests across repositories.
Each component still installs on its own (`cargo install picore-cli`, `picore-mcp`, a GUI package).

## Considered Options

- **One repository per component** (`picor-cli`, `picor-mcp`, `picor-gui`, `picor-vscode`):
  rejected, every change to the index schema would need two or three coordinated PRs.
- **Turborepo** to orchestrate the JS side: rejected, overkill for three packages. A Cargo
  workspace next to a `pnpm-workspace.yaml`, plus a root `justfile` for cross-cutting commands,
  is enough. CI jobs are filtered by path.
