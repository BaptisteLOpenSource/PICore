import { For, Show } from "solid-js";
import { ProductForm } from "./ProductForm";
import { button } from "./ui";
import { deleteProduct, groupByProduct, overview, selectedId, setSelectedId } from "./store";
import type { Listed, ProjectHealth } from "./types";

function badges(h: ProjectHealth | null): { text: string; cls: string }[] {
  if (!h) return [];
  return [
    ...(h.dirty ? [{ text: `dirty ${h.dirty}`, cls: "text-brand" }] : []),
    ...(h.ahead ? [{ text: `ahead ${h.ahead}`, cls: "text-active" }] : []),
    ...(h.behind ? [{ text: `behind ${h.behind}`, cls: "text-active" }] : []),
  ];
}

function Row(props: { project: Listed }) {
  return (
    <button
      type="button"
      aria-pressed={selectedId() === props.project.id}
      onClick={() => setSelectedId(props.project.id)}
      class="flex w-full items-center justify-between gap-2 rounded border border-muted/30 bg-surface px-3 py-2 text-left hover:border-active aria-[pressed=true]:border-active"
    >
      <span class="min-w-0 truncate">{props.project.name}</span>
      <span class="flex shrink-0 items-center gap-2 text-xs">
        <Show when={props.project.branch}>
          {(b) => <span class="font-mono text-muted">{b()}</span>}
        </Show>
        <For each={badges(props.project.health)}>
          {(b) => <span class={b.cls}>{b.text}</span>}
        </For>
      </span>
    </button>
  );
}

function confirmDelete(name: string) {
  if (confirm(`Remove product ${name}? Only its config entry goes, folders are untouched.`)) {
    void deleteProduct(name);
  }
}

export function List() {
  const groups = () => groupByProduct(overview().projects, overview().products);
  return (
    <section aria-label="Projects" class="flex min-w-0 flex-col gap-4">
      <For each={groups()}>
        {(group) => (
          <div class="flex flex-col gap-1">
            <header class="flex items-start justify-between gap-2">
              <div class="min-w-0">
                <h2 class="font-semibold">{group.product ?? "No product"}</h2>
                <Show when={group.path}>
                  {(p) => <p class="truncate font-mono text-xs text-muted">{p()}</p>}
                </Show>
              </div>
              <Show when={group.product} keyed>
                {(name) => (
                  <button type="button" class={button} onClick={() => confirmDelete(name)}>
                    Delete
                  </button>
                )}
              </Show>
            </header>
            <For each={group.projects}>{(project) => <Row project={project} />}</For>
          </div>
        )}
      </For>
      <ProductForm />
    </section>
  );
}
