import { For, type JSX, Show } from "solid-js";
import { ProductForm } from "./ProductForm";
import { Roots } from "./Roots";
import { dangerButton, panel } from "./ui";
import { deleteProduct, machine, overview } from "./store";
import type { Status } from "./types";

const pill: Record<Status, string> = {
  ok: "text-success",
  ko: "text-alert",
  unknown: "text-muted",
};

function Row(props: { title: string; text: string; children: JSX.Element }) {
  return (
    <section class="grid gap-3 border-b border-border py-8 md:grid-cols-[280px_minmax(0,1fr)] md:gap-10">
      <div>
        <h2 class="text-[16px] font-medium">{props.title}</h2>
        <p class="mt-1 text-[13px] text-muted">{props.text}</p>
      </div>
      <div class={`${panel} flex flex-col gap-4 p-5`}>{props.children}</div>
    </section>
  );
}

function confirmDelete(name: string) {
  if (confirm(`Remove product ${name}? Only its config entry goes, folders are untouched.`)) {
    void deleteProduct(name);
  }
}

export function Settings() {
  const products = () => Object.entries(overview().products).sort(([a], [b]) => a.localeCompare(b));
  return (
    <div class="h-full overflow-y-auto px-8 pt-8">
        <h1 class="text-[26px] font-semibold leading-tight tracking-[-0.01em]">Settings</h1>
        <p class="mt-1 text-[13px] text-muted">Changes save to the PICore config at once.</p>

        <Row title="Scan roots" text="Folders PICore walks to find git projects. Removing one drops its projects from the index, never the folders.">
          <Roots />
        </Row>

        <Row title="Products" text="A product groups the projects that live under one folder.">
          <ul class="flex flex-col divide-y divide-rule">
            <For each={products()} fallback={<li class="text-[13px] text-muted">No product yet.</li>}>
              {([name, path]) => (
                <li class="flex min-h-[44px] items-center justify-between gap-4 py-2">
                  <span class="min-w-0">
                    <span class="block text-[13px]">{name}</span>
                    <span class="block truncate font-mono text-[12px] text-muted">{path}</span>
                  </span>
                  <button type="button" class={dangerButton} onClick={() => confirmDelete(name)}>
                    Remove
                  </button>
                </li>
              )}
            </For>
          </ul>
          <ProductForm />
        </Row>

        <Row title="Machine" text="The tools PICore calls. Checked once at start.">
          <ul class="flex flex-col divide-y divide-rule">
            <For each={machine()}>
              {(c) => (
                <li class="flex min-h-[44px] flex-col justify-center py-2">
                  <span class="flex items-center justify-between gap-4 text-[13px]">
                    {c.name}
                    <span class={`text-[12px] ${pill[c.status]}`}>{c.status}</span>
                  </span>
                  <span class="text-[12px] text-muted">{c.message}</span>
                  <Show when={c.status !== "ok" && c.hint}>
                    {(hint) => <span class="text-[12px] text-text">{hint()}</span>}
                  </Show>
                </li>
              )}
            </For>
          </ul>
        </Row>
    </div>
  );
}
