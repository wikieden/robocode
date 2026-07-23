// @vitest-environment jsdom

import { describe, expect, test, vi } from "vitest";

import {
  renderPermissionDock,
  type PermissionDockProjection,
  type PermissionIntent,
} from "../src/components/permission_dock";

const PROJECTION: PermissionDockProjection = {
  workMode: "build",
  permissionLevel: "ask",
  request: {
    id: "approval-shell",
    toolName: "shell",
    title: "Approve shell",
    message: "Core requests scoped permission",
    inputPreview: "rm -rf target && cargo build --release",
    isMutating: true,
    reason: "recursive delete outside the allowlist",
    risk: "high",
    target: {
      kind: "repo_path",
      display: "/workspace/viden/target",
      canonicalRef: "repo://target",
    },
    policyReasonKey: "permission.requires_approval",
    policyReasonArgs: {},
    expiresAt: 1_700_003_600,
    defaultAction: "deny",
    auditId: "audit-shell",
    blockedByPlan: false,
    actions: [
      { kind: "once", available: true, sessionId: null, paths: [], code: null },
      {
        kind: "session",
        available: true,
        sessionId: "session-core",
        paths: [],
        code: null,
      },
      {
        kind: "repo_allowlist",
        available: true,
        sessionId: null,
        paths: ["/workspace/viden/apps/gui"],
        code: null,
      },
      {
        kind: "always",
        available: false,
        sessionId: null,
        paths: [],
        code: "GUI-CORE-003",
      },
      {
        kind: "edit",
        available: false,
        sessionId: null,
        paths: [],
        code: "GUI-CORE-003",
      },
      { kind: "deny", available: true, sessionId: null, paths: [], code: null },
    ],
  },
};

function setup(projection = PROJECTION) {
  document.body.innerHTML = '<main id="dock"></main>';
  const root = document.querySelector<HTMLElement>("#dock")!;
  const send = vi.fn(async (_intent: PermissionIntent) => undefined);
  renderPermissionDock(root, projection, send, "en");
  return { root, send };
}

describe("Permission Dock", () => {
  test("renders exact risk target scope reason input expiry default and audit facts", () => {
    const { root } = setup();
    const dock = root.querySelector<HTMLElement>("[data-permission-dock]")!;
    expect(dock.classList.contains("gperm")).toBe(true);
    expect(dock.classList.contains("dock")).toBe(true);
    expect(dock.textContent).toContain("HIGH");
    expect(dock.textContent).toContain("repo_path");
    expect(dock.textContent).toContain("/workspace/viden/target");
    expect(dock.textContent).toContain("recursive delete outside the allowlist");
    expect(dock.textContent).toContain("rm -rf target && cargo build --release");
    expect(dock.textContent).toContain("1700003600");
    expect(dock.textContent).toContain("deny");
    expect(dock.textContent).toContain("audit-shell");
    expect(dock.getAttribute("role")).toBe("region");
  });

  test("sends only exact typed choices and keeps Always/Edit visibly unavailable", () => {
    const { root, send } = setup();
    root.querySelector<HTMLButtonElement>('[data-permission-action="repo_allowlist"]')!.click();
    expect(send).toHaveBeenCalledWith({
      type: "respond",
      requestId: "approval-shell",
      choice: "repo_allowlist",
      feedback: null,
    });
    for (const kind of ["always", "edit"]) {
      const button = root.querySelector<HTMLButtonElement>(`[data-permission-action="${kind}"]`)!;
      expect(button.disabled).toBe(true);
      expect(button.textContent).toContain("GUI-CORE-003");
    }
  });

  test("Plan mutation denial disables every approval response and transports nothing", () => {
    const projection: PermissionDockProjection = {
      ...PROJECTION,
      workMode: "plan",
      request: {
        ...PROJECTION.request!,
        blockedByPlan: true,
        actions: PROJECTION.request!.actions.map((action) => ({ ...action, available: false })),
      },
    };
    const { root, send } = setup(projection);
    expect(root.querySelector('[role="alert"]')?.textContent).toContain("Plan");
    root.querySelectorAll<HTMLButtonElement>("[data-permission-action]").forEach((button) => {
      button.click();
    });
    expect(send).not.toHaveBeenCalled();
    expect(root.querySelector<HTMLButtonElement>('[data-permission-action="deny"]')!.disabled).toBe(true);
  });

  test("supports keyboard shortcuts and visible focus without color-only status", () => {
    const { root, send } = setup();
    const dock = root.querySelector<HTMLElement>("[data-permission-dock]")!;
    dock.dispatchEvent(new KeyboardEvent("keydown", { key: "y", bubbles: true }));
    expect(send).toHaveBeenCalledWith({
      type: "respond",
      requestId: "approval-shell",
      choice: "once",
      feedback: null,
    });
    const first = root.querySelector<HTMLButtonElement>('[data-permission-action="once"]')!;
    expect(document.activeElement).toBe(first);
    expect(first.getAttribute("aria-keyshortcuts")).toBe("Y");
  });
});
