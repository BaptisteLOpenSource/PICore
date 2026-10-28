# Products are declared in the config, not in the Index

Several Projects can form one Product: `Room-booking/` holds `backend`, `frontend` and `core`,
three git repositories for a single product. PICore names this grouping a **Product**.
"Workspace" was rejected: the glossary already lists it as a word to avoid for Project.

A Product is created by the user (CLI, then GUI), so it is user-entered data. The Index is a cache
that holds no user-entered data and can be rebuilt from git at any time. Products are therefore
stored in `config.toml`, next to the Scan Roots, never only in the Index. The Index may copy
them to answer queries, but it rebuilds that copy from the config.

A Product has a name and, when one exists, a main folder (the parent directory its Projects
share). Each Project shows the Product it belongs to.

## Consequences

- Deleting the Index loses no Product.
- CLI and GUI write the same config: the GUI keeps no state of its own.
- Still open, tracked in #39 and #40: declared vs derived from the parent folder, a Project in
  several Products, key (name or path), exposure in the Project Context over MCP.
