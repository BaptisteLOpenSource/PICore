import { describe, expect, it } from "vitest";
import { colors } from "../../../design/tokens";
import { facts, shortPath, sync } from "./format";
import type { Detail } from "./types";

describe("shortPath", () => {
  it("replaces the home prefix with ~", () => {
    expect(shortPath("/home/me/src/app", "/home/me")).toBe("~/src/app");
    expect(shortPath("/home/me", "/home/me")).toBe("~");
  });

  it("leaves a sibling of home alone", () => {
    expect(shortPath("/home/meow/app", "/home/me")).toBe("/home/meow/app");
  });

  it("drops leading segments so the tail stays visible", () => {
    expect(shortPath("/home/me/Documents/Git/Config/PICore.worktrees/78-fix", "/home/me", 30)).toBe(
      ".../PICore.worktrees/78-fix",
    );
  });

  it("keeps the last segment even when it alone is too long", () => {
    expect(shortPath("/a/very-long-final-segment", null, 10)).toBe(".../very-long-final-segment");
  });
});

describe("sync", () => {
  it("shows nothing when in sync or without upstream", () => {
    expect(sync({ dirty: 3, ahead: 0, behind: 0 })).toBeNull();
    expect(sync({ dirty: 0, ahead: null, behind: null })).toBeNull();
  });

  it("shows only the non-zero side, with a spoken label", () => {
    expect(sync({ dirty: 0, ahead: 2, behind: 0 })).toEqual({ text: "↑2", label: "2 ahead" });
    expect(sync({ dirty: 0, ahead: 2, behind: 1 })).toEqual({ text: "↑2 ↓1", label: "2 ahead, 1 behind" });
  });
});

const detail = (over: Partial<Detail["context"]>, remote: string | null = null): Detail => ({
  context: { name: "app", product: null, branch: "main", techs: [], worktrees: [], files: [], ...over },
  health: {
    worktrees: [],
    remote: remote === null ? null : { name: "remote", status: "ok", message: remote, hint: null },
  },
  prune: [],
});

describe("facts", () => {
  it("hides empty product, remote and techs", () => {
    expect(facts(detail({})).map((f) => f.label)).toEqual(["Branch", "Upstream"]);
  });

  it("keeps filled ones in order", () => {
    const f = facts(detail({ product: "prod", techs: ["rust", "ts"] }, "reachable"));
    expect(f.map((x) => [x.label, x.value])).toEqual([
      ["Product", "prod"],
      ["Branch", "main"],
      ["Upstream", "No upstream"],
      ["Remote", "reachable"],
      ["Techs", "rust, ts"],
    ]);
  });
});

const luminance = (hex: string) => {
  const [r, g, b] = [1, 3, 5]
    .map((i) => parseInt(hex.slice(i, i + 2), 16) / 255)
    .map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

// WCAG 2 contrast ratio of two #rrggbb colors.
function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

// axe needs a DOM runner; tokens are the only colors, so every pair used for text is checked here.
describe("contrast", () => {
  const text = ["text", "muted", "brand", "success", "alert"] as const;
  const grounds = ["bg", "surface", "raised"] as const;
  for (const fg of text)
    for (const bg of grounds)
      it(`${fg} on ${bg} meets WCAG AA`, () => {
        expect(contrast(colors[fg], colors[bg])).toBeGreaterThanOrEqual(4.5);
      });

  it("primary button text on its fill meets WCAG AA", () => {
    expect(contrast(colors.bg, colors.text)).toBeGreaterThanOrEqual(4.5);
  });

  it("matches the WCAG reference values", () => {
    expect(contrast("#FFFFFF", "#000000")).toBeCloseTo(21, 5);
    expect(contrast("#777777", "#FFFFFF")).toBeCloseTo(4.48, 2);
  });
});
