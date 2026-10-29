# Products are declared in the config, not in the Index

Several Projects can form one Product: `Room-booking/` holds `backend`, `frontend` and `core`,
three git repositories for a single product. PICore names this grouping a **Product**.
"Workspace" was rejected: the glossary already lists it as a word to avoid for Project.

A Product is created by the user (CLI, then GUI), so it is user-entered data. The Index is a cache
that holds no user-entered data and can be rebuilt from git at any time. Products are therefore
stored in `config.toml`, next to the Scan Roots, never only in the Index. The Index may copy
them to answer queries, but it rebuilds that copy from the config.

A Product is a unique name plus a main folder: `[products.room-booking] path = "..."`.
Membership is derived, not listed: every Project under that folder belongs to the Product. No
attach or detach command exists, and a Project moved inside the folder stays a member. Product
folders cannot nest, so a Project belongs to at most one Product. Products scattered across
unrelated folders are not supported.

## Consequences

- Deleting the Index loses no Product.
- CLI and GUI write the same config: the GUI keeps no state of its own.
- Nested Product folders or duplicate names fail at config load with a clear error.
- MCP exposes the Product name as a `product` field in `list_projects` and the Project Context.
- No automatic Product suggestion: creating one is a single config entry.
