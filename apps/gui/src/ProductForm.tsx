import { createSignal, Show } from "solid-js";
import { button } from "./ui";
import { chooseFolder, createProduct } from "./store";

export function ProductForm() {
  const [name, setName] = createSignal("");
  const [path, setPath] = createSignal<string | null>(null);
  const ready = () => name().trim() !== "" && path() !== null;

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const folder = path();
    if (!folder || !ready()) return;
    if (await createProduct(name().trim(), folder)) {
      setName("");
      setPath(null);
    }
  }

  return (
    <form onSubmit={submit} class="flex flex-col gap-2 border-t border-slate-800 pt-4">
      <h2 class="font-semibold">New product</h2>
      <label class="flex flex-col gap-1 text-sm text-muted">
        Name
        <input
          value={name()}
          onInput={(e) => setName(e.currentTarget.value)}
          class="rounded border border-slate-800 bg-surface px-2 py-1 text-text"
        />
      </label>
      <div class="flex min-w-0 items-center gap-2">
        <button
          type="button"
          class={button}
          onClick={async () => {
            const folder = await chooseFolder();
            if (folder) setPath(folder);
          }}
        >
          Choose folder
        </button>
        <Show when={path()} keyed>
          {(folder) => <span class="truncate font-mono text-xs">{folder}</span>}
        </Show>
      </div>
      <button type="submit" class={button} disabled={!ready()}>
        Create
      </button>
    </form>
  );
}
