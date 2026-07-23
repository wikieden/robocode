// @vitest-environment node

import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";

import viteConfig from "../vite.config";

const tauriConfigPath = fileURLToPath(
  new URL("../src-tauri/tauri.conf.json", import.meta.url),
);

function readTauriConfig(): {
  build: { devUrl: string };
  bundle: { icon: string[] };
} {
  return JSON.parse(readFileSync(tauriConfigPath, "utf8")) as {
    build: { devUrl: string };
    bundle: { icon: string[] };
  };
}

describe("desktop development bootstrap", () => {
  test("Vite listens on the exact URL awaited by Tauri", () => {
    const tauriConfig = readTauriConfig();
    const devUrl = new URL(tauriConfig.build.devUrl);

    expect(viteConfig.server).toMatchObject({
      host: devUrl.hostname,
      port: Number(devUrl.port),
      strictPort: true,
    });
  });

  test("declares an existing macOS icon for standalone app bundling", () => {
    const tauriConfig = readTauriConfig();
    const macIcon = tauriConfig.bundle.icon.find((icon) => icon.endsWith(".icns"));

    expect(macIcon).toBeDefined();
    expect(
      existsSync(fileURLToPath(new URL(`../src-tauri/${macIcon}`, import.meta.url))),
    ).toBe(true);
  });
});
