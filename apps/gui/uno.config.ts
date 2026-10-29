import { defineConfig, presetWind3 } from "unocss";
import { colors } from "../../design/tokens";

export default defineConfig({
  presets: [presetWind3()],
  theme: { colors },
});
