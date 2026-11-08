import { describe, expect, it } from "vitest";
import { groupByProduct, needsAttention, totals } from "./store";
import type { Listed, WorktreeHealth } from "./types";

const project = (name: string, product: string | null): Listed => ({
  id: `/src/${name}`,
  name,
  work_tree: null,
  branch: "main",
  techs: [],
  worktrees: [],
  stamp: 0,
  product,
  health: null,
});

describe("groupByProduct", () => {
  it("sorts products by name and projects by name inside a group", () => {
    const groups = groupByProduct(
      [project("zeta", "beta"), project("alpha", "beta"), project("solo", "alpha")],
      { beta: "/p/beta", alpha: "/p/alpha" },
    );
    expect(groups.map((g) => g.product)).toEqual(["alpha", "beta"]);
    expect(groups[0].path).toBe("/p/alpha");
    expect(groups[1].projects.map((p) => p.name)).toEqual(["alpha", "zeta"]);
  });

  it("keeps a product with no project so it can be deleted", () => {
    const groups = groupByProduct([project("solo", null)], { empty: "/p/empty" });
    expect(groups[0]).toEqual({ product: "empty", path: "/p/empty", projects: [] });
  });

  it("puts the no-product group last", () => {
    const groups = groupByProduct(
      [project("solo", null), project("app", "prod")],
      { prod: "/p/prod" },
    );
    expect(groups.map((g) => g.product)).toEqual(["prod", null]);
  });

  it("omits the no-product group when empty", () => {
    const groups = groupByProduct([project("app", "prod")], { prod: "/p/prod" });
    expect(groups.map((g) => g.product)).toEqual(["prod"]);
  });
});

const row = (dirty: number, ahead: number | null, behind: number | null): WorktreeHealth => ({
  path: "/w",
  branch: "main",
  dirty,
  ahead,
  behind,
  upstream: ahead === null ? null : "origin/main",
});

describe("totals", () => {
  it("sums every worktree so a dirty linked one counts", () => {
    const h = { worktrees: [row(0, 0, 0), row(2, null, null)], remote: null };
    expect(totals(h)).toEqual({ dirty: 2, ahead: 0, behind: 0 });
    expect(needsAttention(h)).toBe(true);
  });

  it("keeps no upstream as null, not zero", () => {
    const h = { worktrees: [row(0, null, null)], remote: null };
    expect(totals(h)).toEqual({ dirty: 0, ahead: null, behind: null });
    expect(needsAttention(h)).toBe(false);
  });
});
