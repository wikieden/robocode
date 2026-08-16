// @vitest-environment jsdom

import { beforeEach, describe, expect, test, vi } from "vitest";

describe("standalone desktop bootstrap", () => {
  beforeEach(() => {
    vi.resetModules();
    document.body.innerHTML = '<main id="app"></main>';
  });

  // The dynamic import below triggers the first vite transform of the whole
  // D1/D2/D10/D12/D13/D14 screen graph; on a cold transform cache that can
  // exceed the 5s default testTimeout, so this test carries an explicit budget.
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
  }, 30_000);
});
