import { For } from "solid-js";
import { Remove } from "./Remove";
import { button } from "./ui";
import { addRoot, chooseFolder, deleteRoot, home, overview } from "./store";
import { shortPath } from "./format";

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
              <span class="min-w-0 truncate font-mono text-[12px]" title={root}>
                {shortPath(root, home())}
              </span>
              <Remove what={root} onConfirm={() => void deleteRoot(root)} />
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
