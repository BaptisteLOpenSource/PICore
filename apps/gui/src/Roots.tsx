import { For } from "solid-js";
import { button } from "./ui";
import { addRoot, chooseFolder, deleteRoot, overview } from "./store";

function confirmDelete(path: string) {
  if (confirm(`Remove scan root ${path}? Its projects leave the index, folders are untouched.`)) {
    void deleteRoot(path);
  }
}

async function pickRoot() {
  const folder = await chooseFolder();
  if (folder) void addRoot(folder);
}

export function Roots() {
  return (
    <section aria-label="Scan roots" class="flex flex-col gap-2 border-t border-muted/30 pt-4">
      <header class="flex items-center justify-between gap-2">
        <h2 class="font-semibold">Scan roots</h2>
        <button type="button" class={button} onClick={pickRoot}>
          Add root
        </button>
      </header>
      <For each={overview().roots} fallback={<p class="text-sm text-muted">No scan root.</p>}>
        {(root) => (
          <div class="flex min-w-0 items-center justify-between gap-2">
            <span class="truncate font-mono text-xs">{root}</span>
            <button type="button" class={button} onClick={() => confirmDelete(root)}>
              Remove
            </button>
          </div>
        )}
      </For>
    </section>
  );
}
