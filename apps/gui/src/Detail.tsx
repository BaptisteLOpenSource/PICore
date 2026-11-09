import { createSignal, For, Show } from "solid-js";
import { Dynamic } from "solid-js/web";
import { facts, shortPath } from "./format";
import { panel, textButton } from "./ui";
import { copyText, detail, home, isWide, selectedId, setSelectedId, totals } from "./store";
import type { Detail as DetailData } from "./types";

function Copy(props: { text: string; label: string }) {
  const [done, setDone] = createSignal(false);
  return (
    <button
      type="button"
      class={`${textButton} flex-none`}
      onClick={async () => {
        await copyText(props.text);
        setDone(true);
        setTimeout(() => setDone(false), 1500);
      }}
    >
      {done() ? "Copied" : props.label}
    </button>
  );
}

function Info(props: { data: DetailData }) {
  const ctx = () => props.data.context;
  const health = () => props.data.health;
  const figures = () => {
    const t = totals(health());
    return [
      { label: "Changes", value: t.dirty },
      { label: "Ahead", value: t.ahead },
      { label: "Behind", value: t.behind },
    ];
  };
  const pruneCmd = () => `picore prune '${(selectedId() ?? "").replaceAll("'", "'\\''")}'`;

  return (
    <div class="flex flex-col gap-6">
      <header class="min-w-0">
        <Dynamic component={isWide() ? "h2" : "h1"} class="truncate text-[20px] font-semibold leading-tight">
          {ctx().name}
        </Dynamic>
        <p class="mt-1 flex items-baseline justify-between gap-3">
          <span class="min-w-0 truncate font-mono text-[12px] text-muted" title={selectedId() ?? ""}>
            {shortPath(selectedId() ?? "", home())}
          </span>
          <Copy text={selectedId() ?? ""} label="Copy path" />
        </p>
      </header>

      <dl class={`${panel} grid grid-cols-3 divide-x divide-border`}>
        <For each={figures()}>
          {(f) => (
            <div class="px-4 py-3">
              <dt class="text-[12px] text-muted">{f.label}</dt>
              <dd class={`mt-0.5 text-[20px] ${f.value ? "text-text" : "text-muted"}`}>{f.value ?? "-"}</dd>
            </div>
          )}
        </For>
      </dl>

      <dl class="flex flex-col">
        <For each={facts(props.data)}>
          {(f) => (
            <div class="flex min-h-[28px] items-baseline justify-between gap-4 border-b border-rule py-1.5">
              <dt class="shrink-0 text-[12px] text-muted">{f.label}</dt>
              <dd class={`min-w-0 truncate text-right ${f.mono ? "font-mono text-[12px]" : "text-[13px]"}`} title={f.value}>
                {f.value}
              </dd>
            </div>
          )}
        </For>
      </dl>

      <section aria-label="Worktrees">
        <h3 class="mb-2 text-[16px] font-semibold">
          Worktrees <span class="font-normal text-muted">{ctx().worktrees.length}</span>
        </h3>
        <ul class={`${panel} flex flex-col divide-y divide-rule px-4`}>
          <For each={ctx().worktrees}>
            {(w) => {
              const verdict = () => props.data.prune.find((a) => a.path === w.path);
              const error = () => health().worktrees.find((h) => h.path === w.path)?.error;
              return (
                <li class="flex min-h-[44px] flex-col justify-center gap-0.5 py-2">
                  <span class="flex items-center gap-2">
                    <span class="min-w-0 flex-1 truncate font-mono text-[13px]" title={w.path}>
                      {shortPath(w.path, home())}
                    </span>
                    <Show when={w.is_main}>
                      <span class="flex-none rounded bg-raised px-1.5 py-0.5 text-[11px] text-muted">main</span>
                    </Show>
                    <Copy text={w.path} label="Copy path" />
                  </span>
                  <span class="flex items-center gap-2 font-mono text-[12px] text-muted">
                    <span class="min-w-0 truncate" title={w.branch ?? ""}>
                      {w.branch ?? (w.detached ? "detached" : "-")}
                    </span>
                    <Show when={w.issue}>{(n) => <span class="flex-none text-text">#{n()}</span>}</Show>
                  </span>
                  <Show when={error()}>{(e) => <span class="text-[12px] text-alert">{e()}</span>}</Show>
                  <Show when={verdict()?.verdict === "prunable" && verdict()}>
                    {(a) => (
                      <span class="flex items-center justify-between gap-2 text-[12px]">
                        <span class="text-brand">
                          Prunable ({a().reason}): <code class="font-mono text-muted">picore prune</code>
                        </span>
                        <Copy text={pruneCmd()} label="Copy command" />
                      </span>
                    )}
                  </Show>
                </li>
              );
            }}
          </For>
        </ul>
      </section>

      <section aria-label="Files" class="flex flex-col gap-3">
        <h3 class="text-[16px] font-semibold">Agent files</h3>
        <For each={ctx().files} fallback={<p class="text-[13px] text-muted">No README, CLAUDE.md or AGENTS.md.</p>}>
          {(f) => (
            <div class={`${panel} px-4 py-3`}>
              <p class="mb-2 font-mono text-[12px] text-brand">{f.file}</p>
              <ul class="flex flex-col gap-1 text-[13px]">
                <For each={f.headings}>
                  {(h) => (
                    <li
                      class={h.level > 1 ? "text-muted" : ""}
                      style={{ "padding-left": `${(h.level - 1) * 0.75}rem` }}
                    >
                      {h.text}
                    </li>
                  )}
                </For>
              </ul>
              <Show when={f.headings_truncated}>
                <p class="mt-2 text-[12px] text-muted">More headings not shown.</p>
              </Show>
            </div>
          )}
        </For>
      </section>
    </div>
  );
}

const note = (text: string) => <p class="pt-24 text-center text-[13px] text-muted">{text}</p>;

export function Detail() {
  return (
    <section aria-label="Detail" class="h-full overflow-y-auto px-6 py-8">
      <button type="button" class={`${textButton} mb-4 lg:hidden`} onClick={() => setSelectedId(null)}>
        Back to projects
      </button>
      <Show when={selectedId()} fallback={note("Select a project to see its branch, worktrees and agent files.")}>
        <Show
          when={!detail.error && detail()}
          fallback={note(detail.error ? `Couldn't load this project. ${String(detail.error)}` : "Loading...")}
        >
          {(data) => <Info data={data()} />}
        </Show>
      </Show>
    </section>
  );
}
