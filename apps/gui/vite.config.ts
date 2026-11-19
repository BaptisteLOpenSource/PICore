import UnoCSS from "unocss/vite";
import solid from "vite-plugin-solid";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [UnoCSS(), solid()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  test: { environment: "node" },
});
