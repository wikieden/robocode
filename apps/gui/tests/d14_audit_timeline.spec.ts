// @vitest-environment jsdom

import { describe, expect, test, vi } from "vitest";

import {
  renderD14,
  renderD14AuditTimeline,
  type D14AuditProjection,
  type D14AuditTimelineProjection,
  type D14Ports,
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

  test("keeps the raw replay row's epoch readout as the diagnostic value it is", () => {
    const { root } = setup();
    const time = root.querySelector<HTMLElement>("[data-d14-sequence='1'] time")!;
    // Raw mode is the diagnostic event log: its readout is the Core stream's
    // own value and deliberately does not follow audit mode's formatting.
    expect(time.textContent).toBe("1700000001");
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

const AUDIT: D14AuditProjection = {
  outcome: { state: "confirmed", reason: null },
  rows: [
    {
      auditId: "audit-2",
      timestamp: 1_700_000_600,
      actorKind: "operator",
      agentId: null,
      action: "gate.decided",
      objects: [
        { kind: "merge_gate", id: "gate-1" },
        { kind: "task", id: "task-1" },
      ],
      outcome: "success",
      args: [{ key: "outcome", value: "accepted" }],
    },
    {
      auditId: "audit-1",
      timestamp: 1_700_000_500,
      actorKind: "agent",
      agentId: "codex-acp",
      action: "change.reverted",
      objects: [{ kind: "revert", id: "revert-1" }],
      outcome: "denied",
      args: [],
    },
  ],
  nextBefore: "1700000500:audit-1",
  complete: false,
  loaded: true,
  pendingCommandId: null,
  capabilityAvailable: true,
  scope: null,
};

function auditSetup(
  audit: D14AuditProjection = AUDIT,
  ports: Partial<D14Ports> = {},
  raw: D14AuditTimelineProjection | null = null,
) {
  const queryAudit = vi.fn(async () => audit);
  const loadOlderAudit = vi.fn(async () => audit);
  const loadRaw = vi.fn(async () => PAGE);
  const resolved: D14Ports = { queryAudit, loadOlderAudit, loadRaw, ...ports };
  document.body.innerHTML = '<div id="host"></div>';
  const root = document.querySelector<HTMLElement>("#host")!;
  renderD14(root, audit, "en", resolved, raw);
  return { root, ports: resolved, queryAudit, loadOlderAudit, loadRaw };
}

describe("D14 dual mode", () => {
  test("defaults to audit mode and offers the raw diagnostic mode beside it", () => {
    const { root } = auditSetup();
    const audit = root.querySelector<HTMLButtonElement>("[data-d14-mode='audit']");
    const raw = root.querySelector<HTMLButtonElement>("[data-d14-mode='raw']");
    expect(audit?.getAttribute("aria-pressed")).toBe("true");
    expect(raw?.getAttribute("aria-pressed")).toBe("false");
    expect(raw?.textContent).toContain("diagnostic");
  });

  test("renders the raw dotted action key and Core's object chips verbatim", () => {
    const { root } = auditSetup();
    const rows = [...root.querySelectorAll("[data-d14-audit-id]")].map(
      (node) => (node as HTMLElement).dataset.d14AuditId,
    );
    expect(rows).toEqual(["audit-2", "audit-1"]);
    const first = root.querySelector<HTMLElement>("[data-d14-audit-id='audit-2']")!;
    // Core's stable vocabulary is never localized.
    expect(first.textContent).toContain("gate.decided");
    expect(first.querySelector("[data-d14-object='merge_gate:gate-1']")).not.toBeNull();
    expect(first.querySelector("[data-d14-object='task:task-1']")).not.toBeNull();
    expect(first.querySelector("[data-d14-arg='outcome']")?.textContent).toContain("accepted");
    const second = root.querySelector<HTMLElement>("[data-d14-audit-id='audit-1']")!;
    expect(second.dataset.d14Actor).toBe("agent");
    expect(second.textContent).toContain("codex-acp");
    expect(second.dataset.d14Outcome).toBe("denied");
  });

  test("renders an audit timestamp as a readable UTC time, never a raw epoch integer", () => {
    const { root } = auditSetup();
    const time = root.querySelector<HTMLElement>("[data-d14-audit-id='audit-2'] time")!;
    // 1_700_000_600 is 2023-11-14T22:23:20Z. The zone is spelled out because an
    // audit record is evidence compared across machines.
    expect(time.textContent).toBe("2023-11-14 22:23:20 UTC");
    // The machine-readable value stays ISO on the datetime attribute.
    expect(time.getAttribute("datetime")).toBe("2023-11-14T22:23:20.000Z");
  });

  test("a timestamp outside the Date range falls back to Core's raw value", () => {
    const { root } = auditSetup({
      ...AUDIT,
      rows: [{ ...AUDIT.rows[0], timestamp: Number.MAX_SAFE_INTEGER }],
    });
    const time = root.querySelector<HTMLElement>("[data-d14-audit-id='audit-2'] time")!;
    // Blanking the timeline would be worse than showing the unformatted fact.
    expect(time.textContent).toBe(String(Number.MAX_SAFE_INTEGER));
    expect(time.hasAttribute("datetime")).toBe(false);
  });

  test("switching to raw mode loads the replay page and back again re-reads audit", async () => {
    const { root, loadRaw } = auditSetup();
    root.querySelector<HTMLButtonElement>("[data-d14-mode='raw']")!.click();
    await vi.waitFor(() => expect(root.querySelector("[data-d14-sequence]")).not.toBeNull());
    expect(loadRaw).toHaveBeenCalledWith(null);
    expect(root.querySelector("[data-d14-audit-id]")).toBeNull();

    root.querySelector<HTMLButtonElement>("[data-d14-mode='audit']")!.click();
    await vi.waitFor(() => expect(root.querySelector("[data-d14-audit-id]")).not.toBeNull());
  });

  test("load older appends the page Core returned for its own cursor", async () => {
    const older: D14AuditProjection = {
      ...AUDIT,
      rows: [
        ...AUDIT.rows,
        {
          auditId: "audit-0",
          timestamp: 1_700_000_100,
          actorKind: "system",
          agentId: null,
          action: "handoff.created",
          objects: [],
          outcome: "success",
          args: [],
        },
      ],
      nextBefore: null,
      complete: true,
    };
    const loadOlderAudit = vi.fn(async () => older);
    const { root } = auditSetup(AUDIT, { loadOlderAudit });
    root.querySelector<HTMLButtonElement>("[data-d14-audit-more]")!.click();
    await vi.waitFor(() => expect(root.querySelectorAll("[data-d14-audit-id]")).toHaveLength(3));
    expect(loadOlderAudit).toHaveBeenCalledTimes(1);
    expect(root.querySelector("[data-d14-audit-complete]")).not.toBeNull();
    expect(root.querySelector("[data-d14-audit-more]")).toBeNull();
  });

  test("a rejected read shows Core's own reason and keeps no rows", () => {
    const { root } = auditSetup({
      ...AUDIT,
      outcome: { state: "rejected", reason: "audit store is unavailable" },
      rows: [],
      loaded: false,
    });
    expect(root.querySelector("[data-d14-audit-error]")?.textContent).toContain(
      "audit store is unavailable",
    );
    // Absence is not emptiness.
    expect(root.querySelector("[data-d14-audit-empty]")).toBeNull();
  });

  test("only a confirmed empty page renders as empty", () => {
    const { root } = auditSetup({
      ...AUDIT,
      rows: [],
      loaded: true,
      complete: true,
      nextBefore: null,
    });
    expect(root.querySelector("[data-d14-audit-empty]")).not.toBeNull();
  });

  test("an absent capability opens raw mode with a note naming runtime.audit", async () => {
    const { root } = auditSetup(
      { ...AUDIT, rows: [], loaded: false, capabilityAvailable: false },
      {},
      PAGE,
    );
    expect(root.querySelector<HTMLElement>("[data-d14-mode='raw']")?.getAttribute("aria-pressed"))
      .toBe("true");
    expect(root.querySelector("[data-d14-capability-note]")?.textContent).toContain(
      "runtime.audit",
    );
    // Raw mode stays fully usable.
    expect(root.querySelectorAll("[data-d14-sequence]")).toHaveLength(2);
    // The audit mode button is present but cannot be entered.
    const audit = root.querySelector<HTMLButtonElement>("[data-d14-mode='audit']")!;
    expect(audit.disabled).toBe(true);
  });

  test("a scoped view shows a removable chip whose removal re-queries unscoped", async () => {
    const scoped: D14AuditProjection = {
      ...AUDIT,
      scope: { kind: "revert", id: "revert-1" },
    };
    const queryAudit = vi.fn(async () => ({ ...AUDIT, scope: null }));
    const { root } = auditSetup(scoped, { queryAudit });
    const chip = root.querySelector<HTMLButtonElement>("[data-d14-scope-clear]");
    expect(chip?.textContent).toContain("revert");
    expect(chip?.textContent).toContain("revert-1");
    chip!.click();
    await vi.waitFor(() => expect(root.querySelector("[data-d14-scope-clear]")).toBeNull());
    expect(queryAudit).toHaveBeenCalledWith(null);
  });
});
