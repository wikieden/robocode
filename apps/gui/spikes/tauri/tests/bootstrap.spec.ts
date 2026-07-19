import { JSDOM } from "jsdom";
import { describe, expect, test, vi } from "vitest";
import { fixtureProjection } from "../src/app";

describe("production D1 bootstrap", () => {
  test("initializes fixture streaming and history for queue cancel and scroll", async () => {
    const dom = new JSDOM('<div id="app"></div>');
    const root = dom.window.document.querySelector<HTMLElement>("#app")!;
    vi.stubGlobal("document", dom.window.document);
    vi.doMock("@tauri-apps/api/core", () => ({
      invoke: vi.fn(async () => fixtureProjection()),
    }));
    const { bootstrapD1 } = await import("../src/main");
    const app = await bootstrapD1(root, async () => fixtureProjection());

    const composer = root.querySelector<HTMLTextAreaElement>('[data-role="composer"]')!;
    composer.value = "follow-up";
    composer.dispatchEvent(new dom.window.InputEvent("input", { bubbles: true }));
    root.querySelector<HTMLButtonElement>('[data-role="queue-action"]')!.click();
    root.querySelector<HTMLButtonElement>('[data-role="cancel-action"]')!.click();

    const history = root.querySelector<HTMLElement>('[data-role="history-viewport"]')!;
    expect(history.querySelector('[data-history-row="event-1"]')).not.toBeNull();
    history.dispatchEvent(new dom.window.Event("scroll"));

    expect(app.transcript.anchor).toBe("event-1");
    expect(app.actionLog).toEqual([
      "stream:start",
      "queue:follow-up",
      "stream:cancel",
      "history:event-1",
    ]);
  });
});
