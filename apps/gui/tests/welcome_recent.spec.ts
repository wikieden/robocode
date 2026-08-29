// @vitest-environment jsdom

// The Welcome "Recent" section is a client of Core's bounded recent-work
// inventory. Welcome renders only when no workspace is bound, so a recent row
// replaces nothing and needs no switch confirmation — the guarded switch is the
// project picker's job, because that is where a workspace exists to tear down.
import { beforeEach, describe, expect, test, vi } from "vitest";

import { renderWelcomeCenter, type WelcomeRecentWork } from "../src/components/welcome_center";
import type { RecentProjectView, RecentSessionView } from "../src/models/recent_work";

const NOW = Date.UTC(2026, 0, 1);
const EPOCH = Math.floor(NOW / 1000);

const PROJECTS: RecentProjectView[] = [
  {
    canonicalRoot: "/workspace/spatial-lm",
    displayName: "spatial-lm",
    lastUpdatedAt: EPOCH - 3 * 60 * 60,
    latestSessionId: "session-a",
  },
  {
    canonicalRoot: "/workspace/arm-ctrl",
    displayName: "arm-ctrl",
    lastUpdatedAt: EPOCH - 9 * 24 * 60 * 60,
    latestSessionId: "session-c",
  },
];

const SESSIONS: RecentSessionView[] = [
  {
    canonicalRoot: "/workspace/spatial-lm",
    sessionId: "session-a",
    createdAt: EPOCH - 4 * 60 * 60,
    lastUpdatedAt: EPOCH - 3 * 60 * 60,
    messageCount: 4,
    toolCallCount: 1,
    commandCount: 0,
  },
  {
    canonicalRoot: "/workspace/spatial-lm",
    sessionId: "session-b",
    createdAt: EPOCH - 8 * 60 * 60,
    lastUpdatedAt: EPOCH - 7 * 60 * 60,
    messageCount: 2,
    toolCallCount: 0,
    commandCount: 1,
  },
];

function mount(recent?: Partial<WelcomeRecentWork>) {
  document.body.innerHTML = '<main id="app"></main>';
  const root = document.querySelector<HTMLElement>("#app")!;
  const onOpenRecent = vi.fn();
  renderWelcomeCenter(
    root,
    "en",
    vi.fn(),
    recent === undefined
      ? undefined
      : {
          state: { kind: "loaded", projects: PROJECTS, diagnostics: [] },
          sessions: SESSIONS,
          now: NOW,
          onOpenRecent,
          ...recent,
        },
  );
  return { root, onOpenRecent };
}

describe("welcome recent projects", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  test("renders one row per Core project with its age and session count", () => {
    const { root } = mount({});

    const rows = Array.from(root.querySelectorAll<HTMLElement>("[data-recent-project]"));
    // Core's order is preserved; the client never re-sorts the answer.
    expect(rows.map((row) => row.dataset.recentProject)).toEqual([
      "/workspace/spatial-lm",
      "/workspace/arm-ctrl",
    ]);
    expect(rows[0]?.textContent).toContain("spatial-lm");
    expect(rows[0]?.textContent).toContain("3h ago");
    // Counted from the same bounded fact, never from a directory scan.
    expect(rows[0]?.textContent).toContain("2 sessions");
    expect(rows[1]?.textContent).toContain("1w ago");
    expect(rows[1]?.textContent).toContain("0 sessions");
  });

  test("clicking a recent project opens it directly — Welcome has no workspace to replace", () => {
    const { root, onOpenRecent } = mount({});

    root.querySelector<HTMLButtonElement>('[data-recent-project="/workspace/arm-ctrl"]')!.click();

    expect(onOpenRecent).toHaveBeenCalledWith("/workspace/arm-ctrl");
    expect(root.querySelector("[data-picker-confirm]")).toBeNull();
  });

  test("the four read states stay distinct", () => {
    const unbound = mount();
    expect(
      unbound.root.querySelector<HTMLElement>('[data-recent-state="unavailable"]')?.textContent,
    ).toContain("runtime.recent_work");

    const failed = mount({ state: { kind: "failed", reason: "Core refused the read" } });
    expect(
      failed.root.querySelector<HTMLElement>('[data-recent-state="failed"]')?.textContent,
    ).toContain("Core refused the read");

    const loading = mount({ state: { kind: "loading" } });
    expect(loading.root.querySelector('[data-recent-state="loading"]')).not.toBeNull();

    const empty = mount({ state: { kind: "loaded", projects: [], diagnostics: [] } });
    expect(
      empty.root.querySelector<HTMLElement>('[data-recent-state="empty"]')?.textContent,
    ).toBe("No recent projects yet");
    expect(empty.root.querySelector("[data-recent-project]")).toBeNull();
  });

  test("Core's inventory diagnostics render verbatim", () => {
    const { root } = mount({
      state: { kind: "loaded", projects: PROJECTS, diagnostics: ["recent.record_skipped"] },
    });
    expect(
      root.querySelector('[data-recent-diagnostic="recent.record_skipped"]')?.textContent,
    ).toBe("recent.record_skipped");
  });

  test("a rejected open renders the host's own words without losing the rows", async () => {
    const { root } = mount({
      onOpenRecent: vi.fn(() => Promise.reject(new Error("workspace is gone"))),
    });

    root.querySelector<HTMLButtonElement>('[data-recent-project="/workspace/spatial-lm"]')!
      .click();
    for (let hop = 0; hop < 6; hop += 1) await Promise.resolve();

    expect(root.querySelector("[data-open-project-error]")?.textContent).toContain(
      "workspace is gone",
    );
    expect(root.querySelectorAll("[data-recent-project]")).toHaveLength(2);
  });
});
