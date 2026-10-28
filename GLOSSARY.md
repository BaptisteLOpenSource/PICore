# PICore

PICore indexes the git repositories on a developer's machine, groups their worktrees, reports
whether git and GitHub access work, and serves condensed project context to AI agents.

## Language

### Inventory

**Scan Root**:
A directory the user has declared as a place where PICore looks for Projects.
_Avoid_: Root folder, search path, whitelist entry

**Project**:
One git repository found under a Scan Root, bare or not, identified by its git directory.
_Avoid_: Repo (in code and docs), workspace, folder

**Product**:
A named folder the user declares as one product; every Project under that folder belongs to it.
A Project belongs to at most one Product.
_Avoid_: Workspace, group, monorepo

**Worktree**:
An additional working tree of a Project, attached to that Project wherever it lives on disk.
_Avoid_: Ghost folder, checkout, clone

**Prunable Worktree**:
A Worktree whose directory is gone, or whose branch is merged into the default branch with no local changes.
_Avoid_: Obsolete, dead, orphan

**Stale Worktree**:
A Worktree with no recent activity; shown to the user, never pruned on that ground alone.
_Avoid_: Abandoned, old

**Index**:
PICore's local cache of Projects and Worktrees, always rebuildable from git itself.
_Avoid_: Database, registry

### Health

**Health Check**:
One verification of git or GitHub access, whose result is Ok, Ko or Unknown.
_Avoid_: Test, probe, diagnostic

**Machine Health**:
The Health Checks that concern the whole machine: git installed, SSH agent, GitHub CLI authenticated.
_Avoid_: Global status, environment check

**Project Health**:
The Health Checks that concern one Project: remote reachable, ahead/behind, local changes.
_Avoid_: Repo status

**Unknown**:
The result of a Health Check that could not conclude (offline, timeout), never shown as a failure.
_Avoid_: Error, failed

### Context

**Project Context**:
The condensed, whitelisted description of a Project that PICore hands to an AI agent.
_Avoid_: Summary, prompt, dump
