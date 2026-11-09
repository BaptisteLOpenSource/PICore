import type { Totals } from "./store";
import type { Detail } from "./types";

// Tail = the worktree / project name, the part worth seeing; full path goes in a tooltip.
export function shortPath(path: string, home: string | null, max = 48): string {
  const h = home?.replace(/\/$/, "");
  const p = h && (path === h || path.startsWith(`${h}/`)) ? `~${path.slice(h.length)}` : path;
  if (p.length <= max) return p;
  const parts = p.split("/");
  let tail = parts.pop() ?? "";
  while (parts.length > 0 && `.../${parts.at(-1)}/${tail}`.length <= max) tail = `${parts.pop()}/${tail}`;
  return `.../${tail}`;
}

export function sync(t: Totals): { text: string; label: string } | null {
  const sides = [
    [t.ahead, "↑", "ahead"],
    [t.behind, "↓", "behind"],
  ] as const;
  const shown = sides.filter(([n]) => n);
  if (shown.length === 0) return null;
  return {
    text: shown.map(([n, arrow]) => `${arrow}${n}`).join(" "),
    label: shown.map(([n, , word]) => `${n} ${word}`).join(", "),
  };
}

export type Fact = { label: string; value: string; mono: boolean };

export function facts(d: Detail): Fact[] {
  const rows = [
    { label: "Product", value: d.context.product, mono: false },
    { label: "Branch", value: d.context.branch, mono: true },
    { label: "Upstream", value: d.health.worktrees[0]?.upstream ?? "No upstream", mono: true },
    { label: "Remote", value: d.health.remote?.message, mono: false },
    { label: "Techs", value: d.context.techs.join(", "), mono: false },
  ];
  return rows.filter((f): f is Fact => !!f.value);
}
