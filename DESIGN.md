# Design

Visual and CLI identity. Read before any UI, colour or terminal output change.
Stack: GUI = Tauri v2 + SolidJS + UnoCSS (`apps/gui`). Site = Astro Starlight + UnoCSS (`apps/web`, V2).

## Tokens

Single source. GUI and site import the same token set, never redefine values.

| Token | Hex | Use |
|---|---|---|
| `bg` | `#171719` | app ground, true neutral dark (no navy) |
| `surface` | `#131217` | panels, darker than the ground |
| `raised` | `#212023` | selected row, nested chips |
| `border` | `#222224` | structural 1px lines |
| `rule` | `#1b1a1f` | row rules |
| `control` | `#3a3a3e` | control outlines |
| `brand` | `#F59E0B` | mascot, mark, the 1px selection line |
| `success` | `#6EE7B7` | ok status |
| `alert` | `#FB7185` | ko status, destructive actions |
| `text` | `#FFFFFF` | primary text |
| `muted` | `#8A8C92` | secondary text (4.5:1 on surface) |

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
- Style: "data terminal" (tasteskill app sheet). Ground, darker panels, colour on status only.
- One move: every selection (nav, tab) is a 1px amber line as wide as its label. Selected row = `raised` fill.
- Type: Public Sans (bundled, `@fontsource-variable/public-sans`), 13px body, tabular figures.
  Monospace only for paths and branches.
- Shell: 55px icon rail (mark, Projects, Settings), bottom bar under `sm`.
- Projects: table (Changes / Ahead / Behind, right-aligned, zeros grey) + 440px detail pane from `lg`;
  below `lg` one pane at a time. The title line states the finding ("6 of 8 need attention").
- Settings: two-column rows (label + panel): scan roots, products, machine checks.
- Shared classes live in `apps/gui/src/ui.ts`; reset in `uno.config.ts` preflight. UnoCSS scans
  `.ts` only because the config includes it: keep that line.
- No glow, gradient, glass, icon chip.
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
