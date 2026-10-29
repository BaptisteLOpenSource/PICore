# The MCP server ships as `picore mcp`, not as its own binary

Supersedes the install part of ADR 0001: the MCP server no longer installs on its own.

V1 is public. With two binaries, `cargo install picore` gives a CLI without the MCP server, a
second crate must be published, and every client config names a second program. One binary
means one install, one crate and one client config: `"command": "picore", "args": ["mcp"]`.

The cost: tokio and rmcp enter the CLI binary, which grows. Accepted, the CLI is installed once
and size is not a V1 goal.

The crate `picore-mcp` becomes the module `crates/picore-cli/src/mcp/`. It still depends only on
`picore-core`; the rest of the CLI never calls into it. Client configs written by users depend
on this command name, so reversing it later breaks them.
