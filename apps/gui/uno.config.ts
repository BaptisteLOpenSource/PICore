import { defineConfig, presetWind3 } from "unocss";
import { colors } from "../../design/tokens";

// Minimal reset: without it WebKit paints native dark-on-dark buttons.
const reset = `
*,*::before,*::after{box-sizing:border-box;margin:0;padding:0;border:0 solid}
ol,ul{list-style:none}
body{font-size:13px;line-height:1.5;-webkit-font-smoothing:antialiased;
  font-variant-numeric:tabular-nums}
button,input{font:inherit;color:inherit;background:transparent}
button:not(:disabled){cursor:pointer}
:focus-visible{outline:1px solid ${colors.brand};outline-offset:2px}
::selection{background:${colors.brand}55}
::-webkit-scrollbar{width:10px;height:10px}
::-webkit-scrollbar-thumb{background:${colors.raised};border:3px solid ${colors.bg};border-radius:9999px}
`;

export default defineConfig({
  presets: [presetWind3()],
  theme: { colors, fontFamily: { sans: "'Public Sans Variable', system-ui, sans-serif", mono: "ui-monospace, 'JetBrains Mono', monospace" } },
  preflights: [{ getCSS: () => reset }],
  // Default pipeline skips plain .ts, where the shared classes in ui.ts live.
  content: { pipeline: { include: [/\.[jt]sx?($|\?)/, /\.html($|\?)/] } },
});
