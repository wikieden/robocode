// @vitest-environment jsdom

import { describe, expect, test, vi } from "vitest";

import {
  renderD14AuditTimeline,
  type D14AuditTimelineProjection,
} from "../src/screens/d14_audit_timeline";

const PAGE: D14AuditTimelineProjection = {
  rows: [
    {
      sequence: 1,
      streamId: "core",
      kind: "error",
      known: true,
      timestamp: 1_700_000_001,
      projectId: "project-viden",
      laneId: "lane-1",
      sessionId: null,
      taskId: null,
    },
    {
      sequence: 2,
      streamId: "core",
      kind: "unknown",
      known: false,
      timestamp: null,
      projectId: "project-viden",
      laneId: "lane-2",
      sessionId: null,
      taskId: null,
    },
  ],
  nextCursor: "core:2",
  complete: false,
};

function setup(
  projection: D14AuditTimelineProjection = PAGE,
  loadMore = vi.fn(async () => ({
    rows: [
      {
        sequence: 3,
        streamId: "core",
        kind: "assistant_delta",
        known: true,
        timestamp: 1_700_000_003,
        projectId: "project-viden",
        laneId: "lane-1",
        sessionId: null,
        taskId: null,
      },
    ],
    nextCursor: "core:3",
    complete: true,
  })),
) {
  document.body.innerHTML = '<div id="host"></div>';
  const root = document.querySelector<HTMLElement>("#host")!;
  renderD14AuditTimeline(root, projection, "en", loadMore);
  return { root, loadMore };
}

describe("D14 audit timeline", () => {
  test("renders one row per replayed event in Core cursor order", () => {
    const { root } = setup();
    const rows = [...root.querySelectorAll("[data-d14-sequence]")].map(
      (node) => (node as HTMLElement).dataset.d14Sequence,
    );
    expect(rows).toEqual(["1", "2"]);
  });

  test("keeps an undecodable event visible instead of dropping it", () => {
    const { root } = setup();
    const unknown = root.querySelector<HTMLElement>("[data-d14-unknown='true']");
    expect(unknown?.dataset.d14Sequence).toBe("2");
    expect(unknown?.textContent).toContain("unknown event kind");
  });

  test("appends the next page from the Core cursor", async () => {
    const { root, loadMore } = setup();
    root.querySelector<HTMLButtonElement>("[data-d14-more]")!.click();
    await vi.waitFor(() =>
      expect(root.querySelectorAll("[data-d14-sequence]")).toHaveLength(3),
    );
    expect(loadMore).toHaveBeenCalledWith("core:2");
    expect(root.querySelector("[data-d14-complete]")).not.toBeNull();
  });

  test("surfaces a replay failure rather than a shorter complete-looking trail", async () => {
    const failing = vi.fn(async () => {
      throw new Error("Core replay failed: transport closed");
    });
    const { root } = setup(PAGE, failing as never);
    root.querySelector<HTMLButtonElement>("[data-d14-more]")!.click();
    await vi.waitFor(() => expect(root.querySelector("[data-d14-error]")).not.toBeNull());
    expect(root.querySelector("[data-d14-error]")?.textContent).toContain("transport closed");
    // The rows already shown stay; the failure does not fake completeness.
    expect(root.querySelectorAll("[data-d14-sequence]")).toHaveLength(2);
    expect(root.querySelector("[data-d14-complete]")).toBeNull();
  });
});
