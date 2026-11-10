const control =
  "inline-flex h-[34px] shrink-0 items-center justify-center rounded-full px-4 text-[13px] transition-colors " +
  "disabled:cursor-not-allowed disabled:text-muted";

export const button = `${control} border border-control text-text enabled:hover:border-muted enabled:active:bg-raised`;

export const primaryButton = `${control} bg-text text-bg enabled:hover:bg-text/90 disabled:bg-raised`;

export const textButton = "text-[13px] text-muted underline-offset-4 hover:text-text hover:underline";

export const dangerButton = "text-[13px] text-alert underline-offset-4 hover:underline";

export const panel = "rounded-md bg-surface shadow-[0_12px_28px_rgba(0,0,0,.2)]";

export const field =
  "h-[34px] rounded-md border border-control bg-surface px-3 text-[13px] text-text focus:border-brand";

// The style's one move: selection = 1px accent line as wide as its label.
export const selected = "border-b border-transparent aria-[current=page]:border-brand aria-[selected=true]:border-brand";
