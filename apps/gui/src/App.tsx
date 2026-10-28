import { For, Show } from "solid-js";
import { Detail } from "./Detail";
import { List } from "./List";
import { button } from "./ui";
import { error, machine, overview, refresh, refreshing } from "./store";
import type { Status } from "./types";

const chip: Record<Status, string> = {
  ok: "border-success text-success",
  ko: "border-alert text-alert",
  unknown: "border-muted/30 text-muted",
};

export function App() {
  return (
    <main class="flex min-h-screen flex-col gap-4 p-4">
      <section aria-label="Machine health" class="flex flex-wrap gap-2">
        <For each={machine()}>
          {(c) => (
            <span title={c.hint ?? c.message} class={`rounded border px-2 py-0.5 text-xs ${chip[c.status]}`}>
              {c.name}: {c.message}
            </span>
          )}
        </For>
      </section>
      <header class="flex items-center gap-3">
        <h1 class="text-lg font-semibold">PICore</h1>
        <button
          type="button"
          class={`${button} ${refreshing() ? "shadow-lg shadow-brand/40" : ""}`}
          onClick={() => refresh(true)}
        >
          Refresh
        </button>
      </header>
      <Show when={error()} keyed>
        {(msg) => <p role="alert" class="text-sm text-alert">{msg}</p>}
      </Show>
      <For each={overview().warnings}>{(w) => <p class="text-sm text-muted">{w}</p>}</For>
      <div class="grid min-h-0 flex-1 gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)]">
        <List />
        <Detail />
      </div>
    </main>
  );
}
