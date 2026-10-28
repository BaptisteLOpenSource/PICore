import { describe, expect, it } from "vitest";
import { groupByProduct } from "./store";
import type { Listed } from "./types";

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
