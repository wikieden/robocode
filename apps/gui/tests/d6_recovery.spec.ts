// @vitest-environment jsdom

import { describe, expect, test, vi } from "vitest";

import {
  renderD6Recovery,
  type D6RecoveryProjection,
} from "../src/screens/d6_recovery";

const BASE: D6RecoveryProjection = {
  connection: "live",
  state: "empty",
  detail: null,
  hint: null,
  recoverable: false,
  businessSuccessBlocked: false,
  usedTokens: null,
  hardTokenLimit: null,
  missingCapabilities: [],
  actions: [
    { kind: "reconnect", available: false, code: "GUI-CORE-003" },
    { kind: "inspect", available: true, code: "presentation_only" },
    { kind: "restart", available: false, code: "GUI-CORE-003" },
    { kind: "close_lane", available: false, code: "GUI-CORE-003" },
    { kind: "checkpoint", available: false, code: "GUI-CORE-003" },
  ],
};

function setup(projection: D6RecoveryProjection, locale: "en" | "zh-CN" = "en") {
  document.body.innerHTML = '<main id="stage"></main>';
  const root = document.querySelector<HTMLElement>("#stage")!;
  const reconnect = vi.fn(async () => projection);
  const controller = renderD6Recovery(root, projection, reconnect, locale);
  return { root, reconnect, controller };
}

describe("D6 subordinate recovery surface", () => {
  test.each([
    "empty",
    "connecting",
    "disconnected",
    "provider_error",
    "agent_stopped",
    "context_overflow",
    "gate_queue_clear",
    "incompatible_schema",
    "missing_feature_capability",
    "event_gap",
  ] as const)("renders %s with a textual non-color state cue", (state) => {
    const { root } = setup({
      ...BASE,
      state,
      connection: state === "connecting" ? "connecting" : state === "event_gap" ? "recovering" : "live",
      detail: `${state} detail`,
    });
    const surface = root.querySelector<HTMLElement>("[data-d6-state]")!;
    expect(surface.dataset.d6State).toBe(state);
    expect(surface.querySelector("[data-d6-tag]")?.textContent?.length).toBeGreaterThan(0);
    expect(surface.textContent).toContain(`${state} detail`);
    expect(surface.getAttribute("aria-live")).toBe("polite");
  });

  test("renders exact context overflow facts and typed unavailable recovery actions", () => {
    const { root } = setup({
      ...BASE,
      state: "context_overflow",
      businessSuccessBlocked: true,
      usedTokens: 10_001,
      hardTokenLimit: 10_000,
    });
    expect(root.textContent).toContain("10,001");
    expect(root.textContent).toContain("10,000");
    for (const kind of ["restart", "close_lane", "checkpoint"]) {
      const button = root.querySelector<HTMLButtonElement>(`[data-d6-action="${kind}"]`)!;
      expect(button.disabled).toBe(true);
      expect(button.textContent).toContain("GUI-CORE-003");
    }
  });

  test("keeps D6 inside a cockpit work surface and excludes design-page scaffolding", () => {
    const { root } = setup(BASE);
    expect(root.querySelector(".d6-stage")).not.toBeNull();
    expect(root.querySelector(".statebar")).toBeNull();
    expect(root.textContent).not.toContain("D6 concept · draft 01");
  });
});
