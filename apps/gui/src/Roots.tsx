import { For } from "solid-js";
import { button, dangerButton } from "./ui";
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
    <>
      <ul class="flex flex-col divide-y divide-rule">
        <For each={overview().roots} fallback={<li class="text-[13px] text-muted">No scan root.</li>}>
          {(root) => (
            <li class="flex min-h-[44px] items-center justify-between gap-4 py-2">
              <span class="min-w-0 truncate font-mono text-[12px]">{root}</span>
              <button type="button" class={dangerButton} onClick={() => confirmDelete(root)}>
                Remove
              </button>
            </li>
          )}
        </For>
      </ul>
      <div>
        <button type="button" class={button} onClick={pickRoot}>
          Add root
        </button>
      </div>
    </>
  );
}
