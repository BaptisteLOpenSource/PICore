import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { Check, Detail, Overview } from "./types";

export const api = {
  overview: (discover: boolean) => invoke<Overview>("overview", { discover }),
  detail: (id: string) => invoke<Detail>("detail", { id }),
  machine: () => invoke<Check[]>("machine"),
  addProduct: (name: string, path: string) => invoke<void>("add_product", { name, path }),
  removeProduct: (name: string) => invoke<void>("remove_product", { name }),
  addRoot: (path: string) => invoke<void>("add_root", { path }),
  removeRoot: (path: string) => invoke<void>("remove_root", { path }),
  pickFolder: async (): Promise<string | null> => {
    const picked = await open({ directory: true, multiple: false });
    return typeof picked === "string" ? picked : null;
  },
  onConfigChanged: (cb: () => void) => listen("config-changed", () => cb()),
};
