# Changelog

All notable changes to this project are documented here.

## [Unreleased]

### Features

- **core**: Add git adapter and porcelain parsers (#24)
- **core**: Add config and paths (#25)
- **core**: Discover projects under scan roots (#26)
- **core**: Add sqlite index with incremental refresh (#27)
- **core**: Detect project techs (#28)
- **core**: Add health checks (#29)
- **core**: Assess worktrees and add guarded prune (#30)
- **core**: Resolve project and worktree names (#31)
- **cli**: Add root, index and list commands (#32)
- **cli**: Add open, status, worktrees and prune (#33)
- **core**: Build whitelisted project context (#34)
- **mcp**: Serve read-only project context over stdio (#35)
- **cli**: Show prune reason in dry run output
- **cli**: Add shell completions and man page
- **core**: Group related projects under a shared product (#44)
- **gui**: List projects by product and manage products (#48)
- **gui**: Manage scan roots (#52)
- **cli**: Add gui subcommand
- **mcp**: Mark project file content as untrusted

### Bug fixes

- **core**: Block repo-local programs in ls-remote
- **core**: Run ls-remote outside the repo on its origin url
- **cli**: Exit quietly on broken pipe
- **core**: Blank repo-local filter drivers during status (#53)

### Refactor

- **cli**: Serve mcp from picore mcp subcommand

### Documentation

- Init project docs, glossary, adrs and license
- **design**: Add design tokens and identity
- Document install, usage and mcp setup (#36)
- **adr**: Declare products in config
- **cli**: Document stable exit codes

