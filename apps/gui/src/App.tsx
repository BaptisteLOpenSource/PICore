import { createSignal, For, Show } from "solid-js";
import { Detail } from "./Detail";
import { List } from "./List";
import { Settings } from "./Settings";
import { selected } from "./ui";
import { error, isWide, overview, selectedId } from "./store";

type View = "projects" | "settings";

const icons: Record<View, string> = {
  projects: "M3 4h10M3 8h10M3 12h6",
  settings: "M3 5h4m4 0h2M3 11h2m4 0h4M9 3v4M7 9v4",
};

function Mark() {
  return (
    <svg viewBox="0 0 20 20" class="h-5 w-5 text-brand" aria-label="PICore" role="img">
      <circle cx="6" cy="5" r="2" fill="currentColor" />
      <circle cx="6" cy="15" r="2" fill="currentColor" />
      <circle cx="14" cy="8" r="2" fill="currentColor" />
      <path d="M6 7v6M14 10c0 3-8 1-8 3" stroke="currentColor" stroke-width="1.5" fill="none" />
    </svg>
  );
}

export function App() {
  const [view, setView] = createSignal<View>("projects");
  return (
    <div class="flex h-screen flex-col-reverse bg-bg text-text sm:flex-row">
      <nav
        aria-label="Main"
        class="flex shrink-0 items-center justify-around border-t border-border sm:w-[55px] sm:flex-col sm:justify-start sm:gap-2 sm:border-r sm:border-t-0 sm:py-4"
      >
        <span class="hidden sm:mb-4 sm:block">
          <Mark />
        </span>
        <For each={["projects", "settings"] as View[]}>
          {(v) => (
            <button
              type="button"
              title={v === "projects" ? "Projects" : "Settings"}
              aria-current={view() === v ? "page" : undefined}
              onClick={() => setView(v)}
              class={`flex flex-col items-center gap-0.5 px-3 py-2 text-muted hover:text-text aria-[current=page]:text-text sm:last:mt-auto ${selected}`}
            >
              <svg viewBox="0 0 16 16" class="h-4 w-4" aria-hidden="true">
                <path d={icons[v]} stroke="currentColor" stroke-width="1.5" stroke-linecap="round" fill="none" />
              </svg>
              <span class="text-[11px]">{v === "projects" ? "Projects" : "Settings"}</span>
            </button>
          )}
        </For>
      </nav>
      <main class="flex min-h-0 min-w-0 flex-1 flex-col">
        <Show when={error()} keyed>
          {(msg) => (
            <p role="alert" class="border-b border-border px-8 py-2 text-[13px] text-alert">
              {msg}
            </p>
          )}
        </Show>
        <For each={overview().warnings}>
          {(w) => <p class="border-b border-border px-8 py-2 text-[12px] text-muted">{w}</p>}
        </For>
        <Show when={view() === "projects"} fallback={<Settings />}>
          <div class="grid min-h-0 flex-1 lg:grid-cols-[minmax(0,1fr)_440px]">
            <Show when={isWide() || !selectedId()}>
              <div class="min-h-0 min-w-0">
                <List />
              </div>
            </Show>
            <Show when={isWide() || selectedId()}>
              <div class="min-h-0 min-w-0 border-border lg:border-l">
                <Detail />
              </div>
            </Show>
          </div>
        </Show>
      </main>
    </div>
  );
}
