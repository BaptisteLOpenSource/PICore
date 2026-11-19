import { createSignal, Show } from "solid-js";
import { dangerButton, textButton } from "./ui";

// Inline two-step confirm: WebKitGTK webviews may drop window.confirm.
export function Remove(props: { what: string; onConfirm: () => void }) {
  const [asking, setAsking] = createSignal(false);
  return (
    <Show
      when={asking()}
      fallback={
        <button type="button" class={dangerButton} onClick={() => setAsking(true)}>
          Remove
        </button>
      }
    >
      <span role="group" aria-label={`Remove ${props.what}?`} class="flex flex-none items-center gap-3">
        <button type="button" class={dangerButton} ref={(el) => queueMicrotask(() => el.focus())} onClick={props.onConfirm}>
          Confirm remove
        </button>
        <button type="button" class={textButton} onClick={() => setAsking(false)}>
          Cancel
        </button>
      </span>
    </Show>
  );
}
