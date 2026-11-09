import { createResource, createRoot, createSignal } from "solid-js";
import { api } from "./api";
import type { Check, Listed, Overview, ProjectHealth } from "./types";

export type Group = { product: string | null; path: string | null; projects: Listed[] };

export function groupByProduct(projects: Listed[], products: Record<string, string>): Group[] {
  const names = [...new Set([...Object.keys(products), ...projects.flatMap((p) => p.product ?? [])])].sort();
  const groups = new Map<string | null, Group>(
    [...names, null].map((product) => [
      product,
      { product, path: product === null ? null : (products[product] ?? null), projects: [] },
    ]),
  );
  for (const p of projects) groups.get(p.product)?.projects.push(p);
  for (const g of groups.values()) g.projects.sort((a, b) => a.name.localeCompare(b.name));
  return [...groups.values()].filter((g) => g.product !== null || g.projects.length > 0);
}

export type Totals = { dirty: number; ahead: number | null; behind: number | null };

// null = no worktree has an upstream, so "in sync" and "untracked" stay distinct.
export function totals(h: ProjectHealth): Totals {
  const sum = (k: "ahead" | "behind") =>
    h.worktrees.reduce<number | null>((acc, w) => (w[k] === null ? acc : (acc ?? 0) + w[k]), null);
  return { dirty: h.worktrees.reduce((n, w) => n + (w.dirty ?? 0), 0), ahead: sum("ahead"), behind: sum("behind") };
}

export const needsAttention = (h: ProjectHealth | null) => {
  const t = h && totals(h);
  return !!t && (t.dirty + (t.ahead ?? 0) + (t.behind ?? 0) > 0 || h.worktrees.some((w) => w.error !== null));
};

const EMPTY: Overview = { projects: [], products: {}, roots: [], warnings: [] };

export const [overview, setOverview] = createSignal<Overview>(EMPTY);
export const [machine, setMachine] = createSignal<Check[]>([]);
export const [selectedId, setSelectedId] = createSignal<string | null>(null);
export const [error, setError] = createSignal<string | null>(null);
export const [refreshing, setRefreshing] = createSignal(false);

// Below lg one pane shows at a time, so the open project's name is the h1.
export const [isWide, setWide] = createSignal(true);

// Keyed on overview too: a refresh refetches the open detail.
export const [detail] = createRoot(() =>
  createResource(
    () => {
      const id = selectedId();
      return id ? { id, overview: overview() } : false;
    },
    ({ id }) => api.detail(id),
  ),
);

let inFlight = 0;

export async function refresh(discover: boolean) {
  inFlight++;
  setRefreshing(true);
  try {
    const next = await api.overview(discover);
    const selected = selectedId();
    if (selected && !next.projects.some((p) => p.id === selected)) setSelectedId(null);
    // Wide layout: an empty detail pane wastes half the screen, open the first to fix.
    if (!selectedId() && isWide()) {
      setSelectedId((next.projects.find((p) => needsAttention(p.health)) ?? next.projects[0])?.id ?? null);
    }
    setOverview(next);
    setError(null);
  } catch (e) {
    setError(String(e));
  } finally {
    setRefreshing(--inFlight > 0);
  }
}

async function mutate(action: () => Promise<unknown>, discover = false): Promise<boolean> {
  try {
    await action();
  } catch (e) {
    setError(String(e));
    return false;
  }
  await refresh(discover);
  return true;
}

export const createProduct = (name: string, path: string) =>
  mutate(() => api.addProduct(name, path));

export const deleteProduct = (name: string) => mutate(() => api.removeProduct(name));

// Scan roots change which folders are indexed, so both refresh with discovery.
export const addRoot = (path: string) => mutate(() => api.addRoot(path), true);

export const deleteRoot = (path: string) => mutate(() => api.removeRoot(path), true);

export async function chooseFolder(): Promise<string | null> {
  try {
    return await api.pickFolder();
  } catch (e) {
    setError(String(e));
    return null;
  }
}

export function start() {
  const wide = matchMedia("(min-width: 1024px)");
  setWide(wide.matches);
  wide.addEventListener("change", (e) => setWide(e.matches));
  void refresh(false);
  api.machine().then(setMachine, (e) => setError(String(e)));
  window.addEventListener("focus", () => void refresh(false));
  void api.onConfigChanged(() => void refresh(false));
}
