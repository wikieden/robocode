// @vitest-environment jsdom

import { describe, expect, test, vi } from "vitest";

import {
  renderD2Decisions,
  type D2DecisionsProjection,
  type D2IntentResult,
} from "../src/screens/d2_decisions";

const PROJECTION: D2DecisionsProjection = {
  workMode: "build",
  permissionLevel: "standard",
  pendingTotal: 2,
  selectedId: "approval-1",
  groups: [
    {
      kind: "gate",
      unavailable: null,
      items: [
        {
          id: "approval-1",
          kind: "gate",
          title: "Approve fs_write",
          projectId: "project-viden",
          laneId: "lane-gate",
          sessionId: "session-lane-gate",
          taskId: "task-lane-gate",
          risk: "high",
          status: "pending",
          auditId: "audit-gate",
          updatedAt: null,
          expiresAt: 1_700_000_500,
        },
      ],
    },
    {
      kind: "review",
      unavailable: null,
      items: [
        {
          id: "review-1",
          kind: "review",
          title: "review-1",
          projectId: "project-viden",
          laneId: "lane-review",
          sessionId: null,
          taskId: "task-lane-review",
          risk: null,
          status: "pending",
          auditId: "audit-review",
          updatedAt: 1_700_000_200,
          expiresAt: null,
        },
      ],
    },
    {
      kind: "contract",
      unavailable: { key: "d2.contract.noPendingFact", code: "GUI-CORE-013" },
      items: [],
    },
  ],
  detail: {
    id: "approval-1",
    kind: "gate",
    title: "Approve fs_write",
    projectId: "project-viden",
    laneId: "lane-gate",
    taskId: "task-lane-gate",
    auditId: "audit-gate",
    policyReasonKey: "permission.requires_approval",
    blockedByPlan: false,
    context: {
      source: "approval_input_preview",
      text: "fs_write crates/types/src/runtime.rs",
      unavailable: { key: "d2.context.noStructuredDiff", code: "GUI-CORE-012" },
    },
    evidence: [
      {
        id: "evidence-1",
        kind: "check",
        summary: "viden-types check failed",
        path: "evidence/check.json",
        source: "lane-gate",
        timestamp: 1_700_000_150,
      },
    ],
    actions: [
      { kind: "once", available: true, sessionId: null, paths: [], code: null },
      { kind: "deny", available: true, sessionId: null, paths: [], code: null },
    ],
  },
};

function setup(projection: D2DecisionsProjection = PROJECTION) {
  document.body.innerHTML = '<div id="host"></div>';
  const root = document.querySelector<HTMLElement>("#host")!;
  const send = vi.fn(
    async (): Promise<D2IntentResult> => ({
      projection,
      pendingCommandId: "gui-d2-1",
      outcome: { state: "pending", reason: null },
    }),
  );
  const controller = renderD2Decisions(root, projection, send, "en");
  return { root, send, controller };
}

describe("D2 decision center", () => {
  test("renders one queue over every Core decision family", () => {
    const { root } = setup();
    expect(root.querySelector("[data-d2-group='gate']")).not.toBeNull();
    expect(root.querySelector("[data-d2-group='review']")).not.toBeNull();
    expect(root.querySelector("[data-d2-group='contract']")).not.toBeNull();
    expect(root.querySelector("[data-d2-pending-total]")?.textContent).toContain("2");
  });

  test("declares Core contract gaps instead of hiding them", () => {
    const { root } = setup();
    const codes = [...root.querySelectorAll("[data-d2-unavailable]")].map(
      (node) => (node as HTMLElement).dataset.d2Unavailable,
    );
    expect(codes).toContain("GUI-CORE-013");
    expect(codes).toContain("GUI-CORE-012");
  });

  test("renders the Core input preview verbatim and never a synthesized diff", () => {
    const { root } = setup();
    const pane = root.querySelector<HTMLElement>("[data-d2-context]");
    expect(pane?.dataset.d2Context).toBe("approval_input_preview");
    expect(pane?.querySelector(".d2-context-body")?.textContent).toBe(
      "fs_write crates/types/src/runtime.rs",
    );
    expect(root.querySelector(".d2-diff-row")).toBeNull();
  });

  test("a gate action sends the approval intent for the selected request", () => {
    const { root, send } = setup();
    root.querySelector<HTMLButtonElement>("[data-d2-action='once']")!.click();
    expect(send).toHaveBeenCalledWith({
      type: "respond_approval",
      requestId: "approval-1",
      choice: "once",
      feedback: null,
    });
  });

  test("filtering narrows the queue without dropping the decision detail", () => {
    const { root } = setup();
    root.querySelector<HTMLButtonElement>("[data-d2-filter='review']")!.click();
    expect(root.querySelector("[data-d2-group='gate']")).toBeNull();
    expect(root.querySelector("[data-d2-group='review']")).not.toBeNull();
    expect(root.querySelector("[data-d2-detail='approval-1']")).not.toBeNull();
  });

  test("unavailable actions stay visible, disabled, and named by their code", () => {
    const blocked: D2DecisionsProjection = {
      ...PROJECTION,
      detail: {
        ...PROJECTION.detail!,
        id: "review-1",
        kind: "review",
        actions: [
          {
            kind: "accept_review",
            available: false,
            sessionId: null,
            paths: [],
            code: "GUI-CORE-011",
          },
        ],
      },
    };
    const { root } = setup(blocked);
    const button = root.querySelector<HTMLButtonElement>("[data-d2-action='accept_review']");
    expect(button?.disabled).toBe(true);
    expect(button?.textContent).toContain("GUI-CORE-011");
  });
});
