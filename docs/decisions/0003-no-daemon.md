# No daemon

PICore has no background process of its own. The CLI runs short synchronous commands; the MCP
server lives only as long as the agent session that spawned it over stdio. Both read and refresh
the Index (SQLite, WAL mode); the GUI, being a Rust process itself, embeds
the core and watches the filesystem only while it is open. Refreshes are incremental: a cheap
file-timestamp check decides whether a Project needs git to be run again.

A daemon would only serve one case, "state changed while nothing was open", which the
incremental refresh already covers, at the cost of a system service, an IPC protocol, version
skew between processes and concurrent writers.

## Consequences

- The core exposes one `refresh(project)` entry point used by every component, so a daemon can
  be added later without reshaping the others.
- Health Check results are never stored: each command re-checks, under a timeout.
