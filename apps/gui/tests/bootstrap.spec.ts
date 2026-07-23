// @vitest-environment jsdom

import { beforeEach, describe, expect, test, vi } from "vitest";

const invoke = vi.fn();
const openDialog = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: unknown) => invoke(command, args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (options?: unknown) => openDialog(options),
}));

import { bootstrapShell, hydrateShellFromCore } from "../src/main";
import { D1_PROJECTION } from "./support/d1_projection";

const EMPTY_COCKPIT = {
  ...D1_PROJECTION,
  selectedLaneId: null,
  lanes: [],
  liveWork: {
    tasks: [],
    tools: [],
    approvals: [],
    queuedInputs: [],
    evidence: [],
  },
  transcript: [],
  composer: {
    editable: false,
    busy: false,
    canCancel: false,
    canSubmitImmediately: false,
  },
  recovery: {
    ...D1_PROJECTION.recovery,
    state: "empty" as const,
  },
};

describe("production desktop bootstrap", () => {
  beforeEach(() => {
    invoke.mockReset();
    openDialog.mockReset();
  });

  test("renders the D1 cockpit shell while Core connects", () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app");
    if (!root) {
      throw new Error("test root is missing");
    }

    bootstrapShell(root);

    expect(root.dataset.clientState).toBe("connecting");
    expect(root.querySelector("[data-screen]")?.getAttribute("data-screen")).toBe("d1-cockpit");
    expect(root.textContent).toContain("Core connection pending");
    expect(root.querySelector("[data-d6-state]")?.getAttribute("data-d6-state")).toBe(
      "connecting",
    );
    expect(root.querySelector<HTMLTextAreaElement>("[data-composer]")?.disabled).toBe(true);
  });

  test("renders locale and appearance only from a resolved Core projection", () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app");
    if (!root) {
      throw new Error("test root is missing");
    }

    bootstrapShell(root, {
      locale: "zh-CN",
      skin: "ice",
      mode: "light",
      density: "comfy",
      motion: "reduced",
      diagnostics: [],
    });

    expect(document.documentElement.lang).toBe("zh-CN");
    expect(document.documentElement.dataset).toMatchObject({
      skin: "ice",
      mode: "light",
      density: "comfy",
      motion: "reduced",
    });
    expect(root.textContent).toContain("等待 Core 连接");
    expect(root.dataset.clientState).toBe("connecting");
    expect(root.querySelector("[data-screen]")?.getAttribute("data-screen")).toBe("d1-cockpit");
  });

  test("opens a native folder and enters the project cockpit without routing through D11", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app");
    if (!root) {
      throw new Error("test root is missing");
    }
    let workspaceOpen = false;
    openDialog.mockResolvedValue("/workspace/viden-demo");
    invoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "resolved_preferences") return null;
      if (command === "d1_cockpit") return workspaceOpen ? EMPTY_COCKPIT : null;
      if (command === "open_workspace") {
        expect(args).toEqual({ root: "/workspace/viden-demo" });
        workspaceOpen = true;
        return null;
      }
      throw new Error(`unexpected command ${command}`);
    });

    bootstrapShell(root);
    await hydrateShellFromCore(root);

    expect(root.querySelector("[data-screen]")?.getAttribute("data-screen")).toBe("d1-cockpit");
    expect(root.querySelector("[data-d1-welcome]")?.getAttribute("aria-label")).toBe(
      "Welcome to Viden",
    );
    root.querySelector<HTMLButtonElement>("[data-open-project]")?.click();

    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("open_workspace", {
        root: "/workspace/viden-demo",
      });
    });
    expect(openDialog).toHaveBeenCalledWith({
      directory: true,
      multiple: false,
      title: "Open project folder",
    });
    expect(root.dataset.route).toBe("d1");
    expect(root.dataset.clientState).toBe("connected");
    expect(root.querySelector("[data-d1-welcome]")).toBeNull();
    expect(root.querySelector("[data-create-lane]")).not.toBeNull();
    expect(invoke.mock.calls.some(([command]) => command === "d11_poll")).toBe(false);
  });

  test("keeps the welcome center when the native folder picker is cancelled", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app");
    if (!root) {
      throw new Error("test root is missing");
    }
    openDialog.mockResolvedValue(null);
    invoke.mockImplementation(async (command: string) => {
      if (command === "resolved_preferences") return null;
      if (command === "d1_cockpit") return null;
      throw new Error(`unexpected command ${command}`);
    });

    await hydrateShellFromCore(root);

    root.querySelector<HTMLButtonElement>("[data-open-project]")?.click();
    await vi.waitFor(() => expect(openDialog).toHaveBeenCalledTimes(1));

    expect(root.querySelector("[data-d1-welcome]")).not.toBeNull();
    expect(invoke.mock.calls.some(([command]) => command === "open_workspace")).toBe(false);
  });

  test("keeps the D1 shell and shows D6 disconnected when Core bootstrap fails", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app");
    if (!root) {
      throw new Error("test root is missing");
    }
    invoke.mockImplementation(async (command: string) => {
      if (command === "resolved_preferences") return null;
      if (command === "d1_cockpit") throw new Error("Core adapter is not connected");
      throw new Error(`unexpected command ${command}`);
    });

    bootstrapShell(root);
    await hydrateShellFromCore(root);

    expect(root.dataset.clientState).toBe("disconnected");
    expect(root.querySelector("[data-screen]")?.getAttribute("data-screen")).toBe("d1-cockpit");
    expect(root.querySelector("[data-d6-state]")?.getAttribute("data-d6-state")).toBe(
      "disconnected",
    );
  });
});
