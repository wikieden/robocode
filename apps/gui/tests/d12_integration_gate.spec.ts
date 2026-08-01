// @vitest-environment jsdom

import { describe, expect, test, vi } from "vitest";

import {
  renderD12IntegrationGate,
  type D12IntegrationGateProjection,
} from "../src/screens/d12_integration_gate";

const GATE = {
  gateId: "gate-1",
  taskId: "task-lane-3",
  status: "needs_changes",
  gateType: "patch",
  projectId: "project-boss-rush",
  laneId: "lane-3",
  requiresIndependentValidator: true,
  hasValidator: false,
  requiredEvidence: ["replay-regression"],
  evidenceIds: [] as string[],
};

const PROJECTION: D12IntegrationGateProjection = {
  gates: [GATE],
  selectedGateId: "gate-1",
  detail: {
    gate: GATE,
    missingEvidence: ["replay-regression"],
    bounces: [
      {
        bounceId: "bounce-1",
        originalLaneId: "lane-3",
        taskId: "task-lane-3",
        reason: "src/player/dash.gd conflicts with the merged baseline",
        status: "revalidated",
        evidenceIds: [],
      },
    ],
    reverts: [],
    checks: [{ id: "check-1", name: "replay-regression", status: "failed" }],
    actions: [
      { kind: "accept", available: false, code: null },
      { kind: "reject", available: true, code: null },
    ],
  },
  unavailable: [{ key: "d12.conflict.noStructuredHunk", code: "GUI-CORE-015" }],
};

function setup(projection: D12IntegrationGateProjection = PROJECTION) {
  document.body.innerHTML = '<div id="host"></div>';
  const root = document.querySelector<HTMLElement>("#host")!;
  const onSelect = vi.fn();
  renderD12IntegrationGate(root, projection, "en", onSelect);
  return { root, onSelect };
}

describe("D12 integration gate", () => {
  test("shows the conflict banner and the strong-gate policy", () => {
    const { root } = setup();
    expect(root.querySelector<HTMLElement>("[data-d12-banner]")?.dataset.d12Banner).toBe(
      "conflict",
    );
    expect(root.querySelector(".d12-strength")?.textContent).toContain("cannot be bypassed");
    expect(root.querySelector("[data-d12-policy]")?.textContent).toContain(
      "independent validator required",
    );
  });

  test("keeps accept closed and names the evidence Core is still missing", () => {
    const { root } = setup();
    const accept = root.querySelector<HTMLButtonElement>("[data-d12-action='accept']");
    const reject = root.querySelector<HTMLButtonElement>("[data-d12-action='reject']");
    expect(accept?.disabled).toBe(true);
    expect(reject?.disabled).toBe(false);
    expect(root.querySelector("[data-d12-missing]")?.textContent).toContain(
      "replay-regression",
    );
    // No manual-merge escape hatch exists in the rendered action bar.
    expect(root.querySelectorAll("[data-d12-action]")).toHaveLength(2);
  });

  test("renders the bounce timeline back to the origin lane", () => {
    const { root } = setup();
    const bounce = root.querySelector<HTMLElement>("[data-d12-bounce='bounce-1']");
    expect(bounce?.dataset.d12BounceStatus).toBe("revalidated");
    expect(bounce?.textContent).toContain("lane-3");
  });

  test("shows the post-merge rollback once Core records one", () => {
    const { root } = setup({
      ...PROJECTION,
      detail: {
        ...PROJECTION.detail!,
        gate: { ...GATE, status: "merged" },
        missingEvidence: [],
        actions: [
          { kind: "accept", available: false, code: null },
          { kind: "reject", available: false, code: null },
        ],
        reverts: [
          {
            revertId: "revert-1",
            appliedChangeId: "change-1",
            reason: "cancel window regressed",
            restoredPaths: ["src/player/dash.gd"],
            auditId: "audit-revert-1",
            revertedAt: 1_700_000_900,
          },
        ],
      },
    });
    expect(root.querySelector<HTMLElement>("[data-d12-banner]")?.dataset.d12Banner).toBe(
      "resolved",
    );
    expect(root.querySelector("[data-d12-revert='revert-1']")?.textContent).toContain(
      "audit-revert-1",
    );
  });

  test("declares the missing conflict hunk instead of rendering one", () => {
    const { root } = setup();
    expect(
      root.querySelector<HTMLElement>("[data-d12-unavailable]")?.dataset.d12Unavailable,
    ).toBe("GUI-CORE-015");
    expect(root.querySelector(".d12-diff")).toBeNull();
  });
});
