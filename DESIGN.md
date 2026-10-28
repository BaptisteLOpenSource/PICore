# Design

Visual and CLI identity. Read before any UI, colour or terminal output change.
Stack: GUI = Tauri v2 + SolidJS + UnoCSS (`apps/gui`). Site = Astro Starlight + UnoCSS (`apps/web`, V2).

## Tokens

Single source. GUI and site import the same token set, never redefine values.

| Token | Hex | Use |
|---|---|---|
| `bg` | `#090D16` | app/page background (dark, default) |
| `surface` | `#131C2E` | cards, panels |
| `border` | `slate-800` | thin 1px borders |
| `brand` | `#F59E0B` | mascot, primary accent, scan glow |
| `active` | `#2DD4BF` | data flow, live/active state |
| `success` | mint | worktree created |
| `alert` | coral | unmerged changes on `prune` |
| `text` | `#F8FAFC` | primary text |
| `text-muted` | `#94A3B8` | secondary text |

Amber only as signal: mascot, badges, success pulse, key actions. Never as body colour.

## CLI

- Palette 16/256 colours, consistent. Text stays off-white/light grey.
- Amber: mascot and welcome banner (`picore <project>`).
- Mint: success (worktree created).
- Coral: alerts (`picore prune` finds unmerged changes).
- Rule: never drown the user in colour. Plain text is the default.
- Mascot on stderr, TTY only (see `CLAUDE.md`).

## GUI (Tauri + SolidJS)

- Dark by default. Light theme is V2, after the Starlight site.
- Surfaces: subtle transparency, `border-slate-800` 1px borders.
- Glow: very light amber glow during scan actions only.
- Monospace for paths, git hashes, commands. Sans-serif (Inter or Geist) for UI.
- Target: under 40 MB RAM.

## Site (Starlight, V2)

- Developer docs look: dark, clean, close to Vite / Rust / Tailwind docs.
- Pixel or SVG chick logo.
- Code blocks coloured with the amber/obsidian palette.
- Screenshots of GUI and CLI in action.

## Decisions

- Tokens live in one place. Duplicating a hex across GUI and site is a bug.
- UnoCSS over Tailwind (stack per `README.md`).
- Light theme deferred until the site ships.
