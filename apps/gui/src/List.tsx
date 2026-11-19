import { createSignal, For, Show } from "solid-js";
import { shortPath, sync } from "./format";
import { button, selected } from "./ui";
import {
  groupByProduct,
  home,
  machine,
  needsAttention,
  overview,
  refresh,
  refreshing,
  selectedId,
  setSelectedId,
  totals,
} from "./store";
import type { Listed } from "./types";

function finding(projects: Listed[]): string {
  const n = projects.filter((p) => needsAttention(p.health)).length;
  if (!projects.length) return "No project indexed yet. Add a scan root in Settings.";
  if (!n) return `All ${projects.length} projects are clean and in sync.`;
  const t = projects.map((p) => totals(p.health));
  const dirty = t.filter((x) => x.dirty).length;
  const ahead = t.filter((x) => x.ahead).length;
  const behind = t.filter((x) => x.behind).length;
  const broken = projects.filter((p) => p.health.worktrees.some((w) => w.error)).length;
  const parts = [
    dirty && `${dirty} with local changes`,
    ahead && `${ahead} ahead of upstream`,
    behind && `${behind} behind upstream`,
    broken && `${broken} with errors`,
  ].filter(Boolean);
  return `${n} of ${projects.length} need attention: ${parts.join(", ")}.`;
}

function Sync(props: { project: Listed }) {
  const errors = () => props.project.health.worktrees.flatMap((w) => w.error ?? []);
  const s = () => sync(totals(props.project.health));
  return (
    <td class="pl-3 pr-8 text-right">
      <Show
        when={errors().length === 0}
        fallback={
          <span class="text-alert" title={errors().join("\n")}>
            error
          </span>
        }
      >
        <Show when={s()}>{(x) => <span aria-label={x().label}>{x().text}</span>}</Show>
      </Show>
    </td>
  );
}

// Up/Down walk the project buttons, so the list never needs one Tab per row.
function walk(e: KeyboardEvent) {
  if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
  const rows = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>("[data-row]")];
  const next = rows[rows.indexOf(document.activeElement as HTMLElement) + (e.key === "ArrowDown" ? 1 : -1)];
  if (!next) return;
  e.preventDefault();
  next.focus();
}

function Row(props: { project: Listed }) {
  const dirty = () => totals(props.project.health).dirty;
  return (
    <tr
      aria-selected={selectedId() === props.project.id}
      class="relative h-[52px] border-b sm:h-[42px] border-rule hover:bg-raised/50 aria-[selected=true]:bg-raised"
    >
      <td class="max-w-0 w-[40%] pl-8 pr-3">
        <button
          type="button"
          data-row
          onClick={() => setSelectedId(props.project.id)}
          class="block w-full truncate text-left after:absolute after:inset-0 after:content-['']"
        >
          {props.project.name}
        </button>
        <span class="block truncate font-mono text-[12px] text-muted sm:hidden" title={props.project.branch ?? ""}>
          {props.project.branch ?? "-"}
        </span>
      </td>
      <td
        class="hidden max-w-0 truncate px-3 font-mono text-[13px] text-muted sm:table-cell"
        title={props.project.branch ?? ""}
      >
        {props.project.branch ?? "-"}
      </td>
      <td class="px-3 text-right">{dirty() || ""}</td>
      <Sync project={props.project} />
    </tr>
  );
}

export function List() {
  const [onlyAttention, setOnlyAttention] = createSignal(false);
  const projects = () => overview().projects;
  const shown = () => (onlyAttention() ? projects().filter((p) => needsAttention(p.health)) : projects());
  const groups = () =>
    groupByProduct(shown(), overview().products).filter((g) => g.projects.length > 0);
  const tabs = () => [
    { label: "All", count: projects().length, on: false },
    { label: "Needs attention", count: projects().filter((p) => needsAttention(p.health)).length, on: true },
  ];

  return (
    <section aria-label="Projects" class="flex h-full flex-col overflow-y-auto">
      <header class="flex flex-wrap items-start justify-between gap-4 px-8 pt-8">
        <div class="min-w-0">
          <h1 class="text-[26px] font-semibold leading-tight tracking-[-0.01em]">Projects</h1>
          <p class="mt-1 text-[13px] text-muted">{finding(projects())}</p>
        </div>
        <div class="flex items-center gap-5">
          <ul aria-label="Machine health" class="hidden items-center gap-3 md:flex">
            <For each={machine()}>
              {(c) => (
                <li title={c.hint ?? c.message} class="flex items-center gap-1.5 text-[12px] text-muted">
                  <span
                    class={`h-1.5 w-1.5 rounded-full ${c.status === "ok" ? "bg-success" : c.status === "ko" ? "bg-alert" : "bg-control"}`}
                    aria-hidden="true"
                  />
                  {c.name}
                  <span class="sr-only">{c.status}</span>
                </li>
              )}
            </For>
          </ul>
          <button type="button" class={button} disabled={refreshing()} onClick={() => refresh(true)}>
            {refreshing() ? "Scanning..." : "Refresh"}
          </button>
        </div>
      </header>

      <div role="tablist" class="mt-6 flex gap-6 border-b border-border px-8">
        <For each={tabs()}>
          {(t) => (
            <button
              type="button"
              role="tab"
              aria-selected={onlyAttention() === t.on}
              onClick={() => setOnlyAttention(t.on)}
              class={`-mb-px pb-2.5 text-[13px] text-muted hover:text-text aria-[selected=true]:text-text ${selected}`}
            >
              {t.label} <span class="ml-1 text-muted">{t.count}</span>
            </button>
          )}
        </For>
      </div>

      <table class="w-full table-fixed border-collapse text-[13px]" onKeyDown={walk}>
        <thead>
          <tr class="h-10 text-[12px] text-muted">
            <th scope="col" class="w-[40%] pl-8 pr-3 text-left font-normal">Project</th>
            <th scope="col" class="hidden px-3 text-left font-normal sm:table-cell">Branch</th>
            <th scope="col" class="w-20 px-3 text-right font-normal sm:w-24">Changes</th>
            <th scope="col" class="w-24 pl-3 pr-8 text-right font-normal">Sync</th>
          </tr>
        </thead>
        <For each={groups()}>
          {(group) => (
            <tbody>
              <tr>
                <th scope="colgroup" colspan="4" class="pl-8 pr-8 pb-2 pt-5 text-left font-normal">
                  <span class="text-[12px] text-muted">{group.product ?? "No product"}</span>
                  <Show when={group.path}>
                    {(p) => (
                      <span class="ml-2 font-mono text-[11px] text-muted" title={p()}>
                        {shortPath(p(), home())}
                      </span>
                    )}
                  </Show>
                </th>
              </tr>
              <For each={group.projects}>{(project) => <Row project={project} />}</For>
            </tbody>
          )}
        </For>
      </table>
      <Show when={onlyAttention() && shown().length === 0}>
        <p class="mx-8 mt-4 rounded-md border border-dashed border-control px-4 py-2.5 text-[13px] text-muted">
          Nothing to commit, push or pull.
        </p>
      </Show>
    </section>
  );
}
