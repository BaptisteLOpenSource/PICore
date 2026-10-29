import { createSignal } from "solid-js";
import { button, field, primaryButton } from "./ui";
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
    <form onSubmit={submit} class="flex flex-col gap-3 border-t border-rule pt-4">
      <label class="flex flex-col gap-2 text-[13px] font-medium">
        New product name
        <input value={name()} onInput={(e) => setName(e.currentTarget.value)} class={`${field} font-normal`} />
      </label>
      <div class="flex min-w-0 flex-wrap items-center gap-3">
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
        <span class="min-w-0 flex-1 truncate font-mono text-[12px] text-muted">{path() ?? "No folder chosen"}</span>
        <button type="submit" class={primaryButton} disabled={!ready()}>
          Create
        </button>
      </div>
    </form>
  );
}
