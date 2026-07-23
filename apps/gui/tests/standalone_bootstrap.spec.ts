// @vitest-environment jsdom

import { beforeEach, describe, expect, test, vi } from "vitest";

describe("standalone desktop bootstrap", () => {
  beforeEach(() => {
    vi.resetModules();
    document.body.innerHTML = '<main id="app"></main>';
  });

  test("opens the D1 main window shell instead of an onboarding page", async () => {
    await import("../src/main");

    const app = document.querySelector<HTMLElement>("#app");
    expect(app?.dataset.clientState).toBe("disconnected");
    expect(app?.querySelector("[data-screen]")?.getAttribute("data-screen")).toBe("d1-cockpit");
    expect(app?.querySelector("[data-d6-state]")?.getAttribute("data-d6-state")).toBe(
      "disconnected",
    );
    expect(app?.querySelectorAll("button, input, textarea, select").length).toBeGreaterThan(0);
    expect(app?.querySelector<HTMLTextAreaElement>("[data-composer]")?.disabled).toBe(true);
  });
});
