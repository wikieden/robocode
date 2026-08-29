// @vitest-environment jsdom

// The rail is the design's workspace explorer: one `.wsroot` project group with
// the Lanes nested under it and a `＋ Add project…` footer. It renders exactly
// one group because Core supervises exactly one workspace — the design's second
// project and its "Global" bucket are mock data, and fabricating them would put
// lanes on the screen that no Core fact backs.
import { beforeEach, describe, expect, test, vi } from "vitest";

import { renderLaneRail, type LaneRailOptions } from "../src/components/lane_rail";
import { renderD1Cockpit, type D1Intent, type D1IntentResult } from "../src/screens/d1_cockpit";
import { D1_PROJECTION } from "./support/d1_projection";

const PROJECTION = {
  ...D1_PROJECTION,
  topbarSource: { ...D1_PROJECTION.topbarSource!, project: "viden" },
  lanes: [
    ...D1_PROJECTION.lanes,
    {
      id: "lane-review",
      role: "reviewer",
      status: "waiting_approval",
      summary: "Reviewing",
      branch: "codex/lane-review",
    },
  ],
};

function rail(overrides: Partial<LaneRailOptions> = {}): HTMLElement {
  return renderLaneRail({
    projection: PROJECTION,
    locale: "en",
    open: true,
    selectedLaneId: "lane-core",
    onCreateLane: vi.fn(),
    onDismiss: vi.fn(),
    onSelectLane: vi.fn(),
    onRetryAgent: vi.fn(),
    ...overrides,
  });
}

describe("grouped lane rail", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  test("nests the Lanes under one project group named by Core", () => {
    const element = rail();
    document.body.append(element);

    const group = element.querySelector<HTMLElement>("[data-lane-group]")!;
    expect(group.dataset.laneGroup).toBe("/workspace/viden");
    const toggle = group.querySelector<HTMLButtonElement>("[data-lane-group-toggle]")!;
    // Core's published project name, never one derived from the path.
    expect(toggle.textContent).toContain("viden");
    expect(element.querySelector("[data-lane-group-count]")?.textContent).toBe("2");

    const list = element.querySelector<HTMLElement>("#d1-lane-group-lanes")!;
    expect(list.querySelectorAll("[data-lane-id]")).toHaveLength(2);
    // Every Lane row lives inside the group, not beside it.
    for (const lane of element.querySelectorAll("[data-lane-id]")) {
      expect(list.contains(lane)).toBe(true);
    }
    // No fabricated sibling group and no invented global bucket.
    expect(element.querySelectorAll("[data-lane-group]")).toHaveLength(1);
    expect(element.textContent).not.toContain("Global");
  });

  test("without a Core project name the workspace path labels the group", () => {
    const element = rail({
      projection: { ...PROJECTION, topbarSource: { ...PROJECTION.topbarSource!, project: null } },
    });
    expect(
      element.querySelector("[data-lane-group-toggle]")?.textContent,
    ).toContain("/workspace/viden");
  });

  test("the group collapses from local state without hiding the create action", () => {
    const onToggleCollapsed = vi.fn();
    const expanded = rail({ onToggleCollapsed });
    const toggle = expanded.querySelector<HTMLButtonElement>("[data-lane-group-toggle]")!;
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(toggle.getAttribute("aria-controls")).toBe("d1-lane-group-lanes");
    expect(expanded.querySelector<HTMLElement>("#d1-lane-group-lanes")!.hidden).toBe(false);
    toggle.click();
    expect(onToggleCollapsed).toHaveBeenCalledTimes(1);

    const collapsed = rail({ collapsed: true, onToggleCollapsed });
    expect(
      collapsed.querySelector("[data-lane-group-toggle]")?.getAttribute("aria-expanded"),
    ).toBe("false");
    expect(collapsed.querySelector<HTMLElement>("#d1-lane-group-lanes")!.hidden).toBe(true);
    // Creating a Lane stays reachable while the group is folded away.
    expect(collapsed.querySelector("[data-create-lane]")).not.toBeNull();
  });

  test("the per-group create action keeps its name and its handler", () => {
    const onCreateLane = vi.fn();
    const element = rail({ onCreateLane });
    const create = element.querySelector<HTMLButtonElement>("[data-create-lane]")!;

    // The design draws a bare `＋`; the accessible name still says what it does.
    expect(create.getAttribute("aria-label")).toBe("+ New Lane");
    expect(create.title).toBe("+ New Lane");
    expect(create.closest("[data-lane-group] .wsroot")).not.toBeNull();
    create.click();
    expect(onCreateLane).toHaveBeenCalledTimes(1);
  });

  test("an empty project states it instead of rendering an empty group", () => {
    const element = rail({ projection: { ...PROJECTION, lanes: [] } });
    expect(element.querySelector("[data-lane-group-empty]")?.textContent).toBe("No Lanes yet");
    expect(element.querySelector("[data-lane-group-count]")?.textContent).toBe("0");
  });

  test("the Add project footer appears only when a handler can open the picker", () => {
    expect(rail().querySelector("[data-add-project]")).toBeNull();

    const onAddProject = vi.fn();
    const element = rail({ onAddProject });
    const footer = element.querySelector<HTMLButtonElement>("[data-add-project]")!;
    expect(footer.getAttribute("aria-haspopup")).toBe("dialog");
    footer.click();
    expect(onAddProject).toHaveBeenCalledTimes(1);
  });
});

function cockpit(options = {}) {
  document.body.innerHTML = '<main id="app"></main>';
  const root = document.querySelector<HTMLElement>("#app")!;
  const result: D1IntentResult = {
    projection: PROJECTION,
    pendingCommandId: null,
    outcome: { state: "confirmed", reason: null },
  };
  const controller = renderD1Cockpit(
    root,
    PROJECTION,
    vi.fn(async (_intent: D1Intent) => result),
    vi.fn(async () => result),
    undefined,
    undefined,
    { poll: false, ...options },
  );
  return { root, controller };
}

describe("cockpit project picker wiring", () => {
  test("both anchors stay out while the host cannot reach Core", () => {
    const { root, controller } = cockpit();
    expect(root.querySelector("[data-project-selector]")).toBeNull();
    expect(root.querySelector("[data-add-project]")).toBeNull();
    controller.dispose();
  });

  test("the titlebar selector opens the picker with Core's recent-work answer", async () => {
    const loadRecentWork = vi.fn(async () => ({
      outcome: { state: "confirmed" as const, reason: null },
      projects: [
        {
          canonicalRoot: "/workspace/spatial-lm",
          displayName: "spatial-lm",
          lastUpdatedAt: 1,
          latestSessionId: null,
        },
      ],
      sessions: [],
      diagnostics: [],
      pendingCommandId: null,
      capabilityAvailable: true,
    }));
    const { root, controller } = cockpit({
      loadRecentWork,
      onPickProjectFolder: vi.fn(async () => null),
      onOpenWorkspace: vi.fn(async () => {}),
    });

    const selector = root.querySelector<HTMLButtonElement>("[data-project-selector]")!;
    expect(selector.getAttribute("aria-expanded")).toBe("false");
    selector.click();
    for (let hop = 0; hop < 8; hop += 1) await Promise.resolve();

    expect(loadRecentWork).toHaveBeenCalledTimes(1);
    expect(document.querySelector("[data-project-picker]")).not.toBeNull();
    expect(
      document.querySelector('[data-picker-recent="/workspace/spatial-lm"]'),
    ).not.toBeNull();
    controller.dispose();
    expect(document.querySelector("[data-project-picker]")).toBeNull();
  });

  test("the rail footer opens the same picker and returns focus to itself", async () => {
    const { root, controller } = cockpit({
      loadRecentWork: vi.fn(async () => ({
        outcome: { state: "confirmed" as const, reason: null },
        projects: [],
        sessions: [],
        diagnostics: [],
        pendingCommandId: null,
        capabilityAvailable: true,
      })),
      onPickProjectFolder: vi.fn(async () => null),
      onOpenWorkspace: vi.fn(async () => {}),
    });

    const footer = root.querySelector<HTMLButtonElement>("[data-add-project]")!;
    footer.click();
    for (let hop = 0; hop < 8; hop += 1) await Promise.resolve();
    const picker = document.querySelector<HTMLElement>("[data-project-picker]")!;
    expect(picker).not.toBeNull();

    picker.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(document.querySelector("[data-project-picker]")).toBeNull();
    expect(
      (document.activeElement as HTMLElement | null)?.dataset.addProject,
    ).toBe("true");
    controller.dispose();
  });

  test("a still-pending read is reported as unanswered, not as an empty history", async () => {
    const { root, controller } = cockpit({
      loadRecentWork: vi.fn(async () => ({
        outcome: { state: "pending" as const, reason: null },
        projects: [],
        sessions: [],
        diagnostics: [],
        pendingCommandId: "gui-recent-1",
        capabilityAvailable: true,
      })),
      onPickProjectFolder: vi.fn(async () => null),
      onOpenWorkspace: vi.fn(async () => {}),
    });

    root.querySelector<HTMLButtonElement>("[data-project-selector]")!.click();
    for (let hop = 0; hop < 8; hop += 1) await Promise.resolve();

    expect(
      document.querySelector<HTMLElement>('[data-picker-recent-state="failed"]')?.textContent,
    ).toContain("has not answered");
    controller.dispose();
  });
});
