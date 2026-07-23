import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    host: "localhost",
    port: 1420,
    strictPort: true,
  },
  test: {
    include: ["tests/**/*.spec.ts"],
  },
});
