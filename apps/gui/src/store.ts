import { createResource, createRoot, createSignal } from "solid-js";
import { api } from "./api";
import type { Check, Detail, Listed, Overview } from "./types";

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

const EMPTY: Overview = { projects: [], products: {}, warnings: [] };

export const [overview, setOverview] = createSignal<Overview>(EMPTY);
export const [machine, setMachine] = createSignal<Check[]>([]);
export const [selectedId, setSelectedId] = createSignal<string | null>(null);
export const [error, setError] = createSignal<string | null>(null);
export const [refreshing, setRefreshing] = createSignal(false);

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
    setOverview(await api.overview(discover));
    setError(null);
  } catch (e) {
    setError(String(e));
  } finally {
    setRefreshing(--inFlight > 0);
  }
}

async function mutate(action: () => Promise<unknown>): Promise<boolean> {
  try {
    await action();
  } catch (e) {
    setError(String(e));
    return false;
  }
  await refresh(false);
  return true;
}

export const createProduct = (name: string, path: string) =>
  mutate(() => api.addProduct(name, path));

export const deleteProduct = (name: string) => mutate(() => api.removeProduct(name));

export async function chooseFolder(): Promise<string | null> {
  try {
    return await api.pickFolder();
  } catch (e) {
    setError(String(e));
    return null;
  }
}

export function start() {
  void refresh(false);
  api.machine().then(setMachine, (e) => setError(String(e)));
  window.addEventListener("focus", () => void refresh(false));
  void api.onConfigChanged(() => void refresh(false));
}
