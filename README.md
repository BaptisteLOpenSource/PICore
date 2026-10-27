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

## Planned usage

> Nothing is released yet. Commands below describe the MVP target.

```sh
picore root add ~/code        # declare where your projects live
picore index                  # find projects and their worktrees
picore list                   # projects, branches, worktree counts, techs
picore open api               # open a project in your editor
picore status --remote        # git, ssh-agent, gh, and per-project health
picore prune                  # dry run: what can be removed safely
picore prune --yes            # remove merged, clean worktrees
```

## Documentation

- [Architecture](ARCHITECTURE.md)
- [Glossary](GLOSSARY.md)
- [Decisions](docs/decisions/)

## License

[MIT](LICENSE) © 2026 Baptiste Dechamp
