// @vitest-environment jsdom

import { afterEach, describe, expect, test, vi } from "vitest";

import { renderCockpitTopbar } from "../src/components/cockpit_topbar";
import {
  renderD1Cockpit,
  type D1CockpitProjection,
  type D1IntentResult,
} from "../src/screens/d1_cockpit";

function projection(overrides: Partial<D1CockpitProjection> = {}): D1CockpitProjection {
  return {
    preferences: {
      locale: "en",
      skin: "aurora",
      mode: "dark",
      density: "regular",
      motion: "reduced",
      diagnostics: [],
    },
    selectedLaneId: null,
    contextDock: {
      source: null,
      context: null,
      laneAgent: null,
      provider: null,
      services: [],
      checklist: [],
    },
    lanes: [],
    environment: {
      cwd: "/workspace/viden",
      providerId: "fallback",
      model: "test-local",
      workMode: "build",
      permissionLevel: "ask",
      tokenTotal: 0,
      costMicroUsd: null,
    },
    liveWork: { tasks: [], tools: [], approvals: [], queuedInputs: [], evidence: [] },
    transcript: [],
    workspaceEligibility: null,
    starterLanePreviews: [],
    agentAdapters: [],
    agentSessions: [],
    composer: {
      editable: true,
      busy: false,
      canCancel: false,
      canSubmitImmediately: true,
    },
    permissionDock: { workMode: "build", permissionLevel: "ask", request: null },
    recovery: {
      connection: "live",
      state: "live",
      detail: null,
      hint: null,
      recoverable: false,
      businessSuccessBlocked: false,
      usedTokens: null,
      hardTokenLimit: null,
      missingCapabilities: [],
      actions: [],
    },
    unavailableFeatures: [],
    ...overrides,
  } as D1CockpitProjection;
}

function laneProjection(laneId: string, rows: number): D1CockpitProjection {
  return projection({
    selectedLaneId: laneId,
    lanes: [
      {
        id: laneId,
        summary: `${laneId} active`,
        status: "running",
        role: "coder",
        route: "built_in",
        branch: null,
        worktree: null,
        awaitingHuman: false,
      },
    ] as never,
    transcript: Array.from({ length: rows }, (_, index) => ({
      id: `row-${index}`,
      kind: index % 2 === 0 ? "user" : "assistant",
      content: `content-${index}`,
    })),
  });
}

function idleResult(next: D1CockpitProjection): D1IntentResult {
  return {
    projection: next,
    pendingCommandId: null,
    outcome: { state: "idle", reason: null },
  } as D1IntentResult;
}

function mount(
  initial: D1CockpitProjection,
  poll = vi.fn(async (..._args: unknown[]) => idleResult(projection())),
  options = {},
) {
  document.body.innerHTML = '<div id="app"></div>';
  const root = document.querySelector<HTMLElement>("#app")!;
  const controller = renderD1Cockpit(
    root,
    initial,
    vi.fn(async () => idleResult(initial)),
    poll,
    undefined,
    undefined,
    { poll: false, ...options },
  );
  return { root, controller, poll };
}

afterEach(() => {
  delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  vi.useRealTimers();
});

describe("window chrome", () => {
  test("the HTML traffic lights render only outside the native shell", () => {
    const inBrowser = renderCockpitTopbar(projection(), "en", false);
    expect(inBrowser.element.querySelector(".tl")).not.toBeNull();

    (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    const inTauri = renderCockpitTopbar(projection(), "en", false);
    // macOS supplies the native overlay controls; a second HTML set is a
    // duplicated-chrome defect.
    expect(inTauri.element.querySelector(".tl")).toBeNull();
  });
});

describe("composer lane notices", () => {
  test("a zero-lane project invites lane creation instead of reporting a stale lane", () => {
    const { root } = mount(projection());
    const notice = root.querySelector("[data-mutation-blocked]");
    expect(notice?.textContent).not.toContain("no longer available");
    expect(notice?.textContent).toContain("Create or select a Lane");
  });

  test("a selection that vanished from a project that still has lanes stays fail-closed", () => {
    const { root, controller } = mount(laneProjection("lane-1", 2));
    const survivor = laneProjection("lane-2", 0);
    survivor.selectedLaneId = null;
    controller.applyProjection(survivor);
    // lane-1 is gone but lane-2 exists: keep the honest stale-lane block
    // rather than silently retargeting the composer.
    expect(root.querySelector("[data-mutation-blocked]")?.textContent).toContain(
      "no longer available",
    );
  });

  test("a project with zero lanes clears the vanished selection", () => {
    const { root, controller } = mount(laneProjection("lane-1", 2));
    const emptied = projection();
    controller.applyProjection(emptied);
    const notice = root.querySelector("[data-mutation-blocked]");
    expect(notice?.textContent).not.toContain("no longer available");
    expect(notice?.textContent).toContain("Create or select a Lane");
  });
});

describe("transcript scroll", () => {
  function geometry(region: HTMLElement, scrollTop: number): void {
    Object.defineProperty(region, "scrollHeight", { value: 1000, configurable: true });
    Object.defineProperty(region, "clientHeight", { value: 200, configurable: true });
    region.scrollTop = scrollTop;
  }

  test("a reader scrolled into history keeps their position across a Core refresh", () => {
    const { root, controller } = mount(laneProjection("lane-1", 40));
    const region = root.querySelector<HTMLElement>(".d1-transcript")!;
    geometry(region, 300);
    region.dispatchEvent(new Event("scroll"));
    expect(controller.transcript.followLatest).toBe(false);

    const next = laneProjection("lane-1", 41);
    controller.applyProjection(next);

    const refreshed = root.querySelector<HTMLElement>(".d1-transcript")!;
    // The refresh must not teleport the reader back to an edge.
    expect(refreshed.scrollTop).toBe(300);
    expect(controller.transcript.followLatest).toBe(false);
  });

  test("a reader at the bottom keeps following the newest output", () => {
    const { root, controller } = mount(laneProjection("lane-1", 40));
    const region = root.querySelector<HTMLElement>(".d1-transcript")!;
    geometry(region, 800);
    region.dispatchEvent(new Event("scroll"));
    expect(controller.transcript.followLatest).toBe(true);

    const next = laneProjection("lane-1", 41);
    controller.applyProjection(next);

    const refreshed = root.querySelector<HTMLElement>(".d1-transcript")!;
    expect(refreshed.scrollTop).toBe(refreshed.scrollHeight);
    expect(controller.transcript.followLatest).toBe(true);
  });
});

describe("poll cadence", () => {
  test("the background poll long-polls Core instead of draining an empty queue", async () => {
    vi.useFakeTimers();
    const poll = vi.fn(async (..._args: unknown[]) => idleResult(projection()));
    const { controller } = mount(projection(), poll, { poll: true });
    await vi.advanceTimersByTimeAsync(300);
    expect(poll).toHaveBeenCalled();
    // waitForEvent=true lets the Rust side hold the poll until an ordered
    // event arrives, so a reply renders when it happens rather than on the
    // next timer tick.
    expect(poll.mock.calls[0][1]).toBe(true);
    controller.dispose();
  });
});

describe("core event wake", () => {
  test("a Core wake refreshes immediately instead of waiting for the next tick", async () => {
    vi.useFakeTimers();
    const listeners: Array<() => void> = [];
    const poll = vi.fn(async (..._args: unknown[]) => idleResult(projection()));
    const { controller } = mount(projection(), poll, {
      poll: true,
      onCoreWake: (handler: () => void) => {
        listeners.push(handler);
        return () => undefined;
      },
    });
    expect(listeners).toHaveLength(1);

    listeners[0]();
    await vi.advanceTimersByTimeAsync(0);
    // The wake drives the read; no timer had to elapse first.
    expect(poll).toHaveBeenCalledTimes(1);
    controller.dispose();
  });

  test("the drain timer stops once Core pushes wakes", async () => {
    vi.useFakeTimers();
    const poll = vi.fn(async (..._args: unknown[]) => idleResult(projection()));
    const { controller } = mount(projection(), poll, {
      poll: true,
      onCoreWake: () => () => undefined,
    });
    await vi.advanceTimersByTimeAsync(2000);
    // With a push subscription the shell must not keep its own 250ms drain
    // loop; that loop is the fallback for hosts without the wake.
    expect(poll).not.toHaveBeenCalled();
    controller.dispose();
  });
});
