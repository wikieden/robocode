import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";

const spikeRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const shellRoot = resolve(spikeRoot, "src-tauri");

describe("real Tauri shell", () => {
  test("declares a runnable desktop crate and bridge", () => {
    const requiredFiles = [
      "Cargo.toml",
      "build.rs",
      "tauri.conf.json",
      "src/lib.rs",
      "src/main.rs",
      "src/bridge.rs",
    ];

    expect(requiredFiles.filter((file) => !existsSync(resolve(shellRoot, file)))).toEqual([]);
  });

  test("pins the D1 shell identity and frontend build commands", () => {
    const configPath = resolve(shellRoot, "tauri.conf.json");
    expect(existsSync(configPath)).toBe(true);
    if (!existsSync(configPath)) {
      return;
    }
    const config = JSON.parse(readFileSync(configPath, "utf8"));

    expect(config.productName).toBe("Viden D1 Spike");
    expect(config.identifier).toBe("dev.viden.gui.spike.tauri");
    expect(config.build.beforeBuildCommand).toBe("npm run build");
    expect(config.build.frontendDist).toBe("../dist");
    expect(config.bundle.active).toBe(false);
  });
});
