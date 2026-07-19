import { defineConfig } from "vite";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const spikeRoot = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  clearScreen: false,
  server: {
    fs: {
      allow: [resolve(spikeRoot, "../../../..")],
    },
  },
});
