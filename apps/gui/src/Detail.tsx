import { For, Show } from "solid-js";
import { detail, overview, selectedId } from "./store";
import type { Detail as DetailData } from "./types";

function Info(props: { data: DetailData }) {
  const ctx = () => props.data.context;
  const health = () => props.data.health;
  const product = () => overview().projects.find((p) => p.id === selectedId())?.product;
  const productPath = () => {
    const name = product();
    return name ? overview().products[name] : undefined;
  };
  return (
    <div class="flex flex-col gap-4">
      <div>
        <h2 class="text-lg font-semibold">{ctx().name}</h2>
        <p class="truncate font-mono text-xs text-muted">{selectedId()}</p>
      </div>
      <Show when={product()}>
        {(name) => (
          <p class="text-sm">
            Product <span class="font-semibold">{name()}</span>
            <Show when={productPath()}>
              {(p) => <span class="block truncate font-mono text-xs text-muted">{p()}</span>}
            </Show>
          </p>
        )}
      </Show>
      <p class="text-sm">Branch <span class="font-mono">{ctx().branch ?? "-"}</span></p>
      <p class="text-sm">Techs {ctx().techs.join(", ") || "-"}</p>
      <section aria-label="Worktrees" class="flex flex-col gap-1">
        <h3 class="font-semibold">Worktrees</h3>
        <For each={ctx().worktrees}>
          {(w) => (
            <p class="text-sm">
              <span class="block truncate font-mono text-xs">{w.path}</span>
              <span class="font-mono text-xs text-muted">{w.branch ?? (w.detached ? "detached" : "-")}</span>
            </p>
          )}
        </For>
      </section>
      <section aria-label="Health" class="flex flex-col gap-1 text-sm">
        <h3 class="font-semibold">Health</h3>
        <p>dirty {health().dirty} · ahead {health().ahead} · behind {health().behind}</p>
        <p class="font-mono text-xs text-muted">{health().upstream ?? "no upstream"}</p>
      </section>
      <section aria-label="Files" class="flex flex-col gap-3">
        <h3 class="font-semibold">Files</h3>
        <For each={ctx().files}>
          {(f) => (
            <div class="flex flex-col gap-1">
              <p class="font-mono text-xs">{f.file}</p>
              <ul class="flex flex-col gap-0.5 text-sm">
                <For each={f.headings}>
                  {(h) => <li style={{ "padding-left": `${(h.level - 1) * 0.75}rem` }}>{h.text}</li>}
                </For>
              </ul>
              <Show when={f.headings_truncated}>
                <p class="text-xs text-muted">more headings not shown</p>
              </Show>
            </div>
          )}
        </For>
      </section>
    </div>
  );
}

export function Detail() {
  return (
    <section aria-label="Detail" class="min-w-0">
      <Show when={selectedId()} fallback={<p class="text-sm text-muted">Select a project.</p>}>
        <Show
          when={!detail.error && detail()}
          fallback={<p class="text-sm text-muted">{detail.error ? String(detail.error) : "Loading..."}</p>}
        >
          {(data) => <Info data={data()} />}
        </Show>
      </Show>
    </section>
  );
}
